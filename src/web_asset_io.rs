use bevy::{
    asset::{AssetIo, AssetIoError, BoxedFuture, ChangeWatcher, Metadata},
    utils::hashbrown::HashMap,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

/// Wraps the default bevy AssetIo and adds support for loading http urls
pub struct WebAssetIo {
    pub(crate) default_io: Box<dyn AssetIo>,
    pub(crate) headers: Arc<RwLock<HashMap<String, String>>>,
    pub(crate) cache_name: String,
}

fn is_http(path: &Path) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
}

impl AssetIo for WebAssetIo {
    fn load_path<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<Vec<u8>, AssetIoError>> {
        if is_http(path) {
            let uri = path.to_str().unwrap();

            let headers = { self.headers.read().unwrap().clone() };

            #[cfg(target_arch = "wasm32")]
            let fut = Box::pin(async move {
                use wasm_bindgen::JsCast;
                use wasm_bindgen_futures::JsFuture;
                let window = web_sys::window().unwrap();

                let request = web_sys::Request::new_with_str(uri).unwrap();

                for (name, value) in headers {
                    request.headers().set(&name, &value).unwrap();
                }

                let cached_response =
                    wasm_functions::get_chache_and_set_header(&request, &self.cache_name, uri)
                        .await;

                let response = JsFuture::from(window.fetch_with_request(&request))
                    .await
                    .map(|r| r.dyn_into::<web_sys::Response>().unwrap())
                    .map_err(|e| e.dyn_into::<js_sys::TypeError>().unwrap());

                if let Err(_) = &response {
                    // warn!("Failed to fetch asset {uri}: {err:?}");
                }

                let mut response =
                    response.map_err(|_| AssetIoError::NotFound(path.to_path_buf()))?;

                if let (Some(cached), 304) = (cached_response, response.status()) {
                    response = cached.clone().unwrap();
                } else {
                    let cloned_response = response.clone().unwrap();

                    wasm_functions::save_response_to_cache(
                        &request,
                        &cloned_response,
                        &self.cache_name,
                    )
                    .await;
                }

                let data = JsFuture::from(response.array_buffer().unwrap())
                    .await
                    .unwrap();

                let bytes = js_sys::Uint8Array::new(&data).to_vec();

                Ok(bytes)
            });

            #[cfg(not(target_arch = "wasm32"))]
            let fut = Box::pin(async move {
                let bytes = surf::get(uri)
                    .await
                    .map_err(|_| AssetIoError::NotFound(path.to_path_buf()))?
                    .body_bytes()
                    .await
                    .map_err(|_| AssetIoError::NotFound(path.to_path_buf()))?;
                Ok(bytes)
            });

            fut
        } else {
            self.default_io.load_path(path)
        }
    }

    fn read_directory(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item = PathBuf>>, AssetIoError> {
        self.default_io.read_directory(path)
    }

    fn watch_path_for_changes(
        &self,
        to_watch: &Path,
        to_reload: Option<PathBuf>,
    ) -> Result<(), AssetIoError> {
        if is_http(to_watch) {
            // TODO: we could potentially start polling over http here
            // but should probably only be done if the server supports caching

            // This is where we would write to a self.network_watcher

            Ok(()) // Pretend everything is fine
        } else {
            self.default_io.watch_path_for_changes(to_watch, to_reload)
        }
    }

    fn watch_for_changes(&self, configuration: &ChangeWatcher) -> Result<(), AssetIoError> {
        self.default_io.watch_for_changes(configuration)
    }

    fn is_dir(&self, path: &Path) -> bool {
        if is_http(path) {
            false
        } else {
            self.default_io.is_dir(path)
        }
    }

    fn get_metadata(&self, path: &Path) -> Result<Metadata, AssetIoError> {
        self.default_io.get_metadata(path)
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_functions {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Cache, Request, Response};
    /// Sets the `If-None-Match` header if a previous response is in the cache and contains an etag
    ///
    /// returns the response if one exists
    pub(super) async fn get_chache_and_set_header(
        request: &Request,
        cache_name: &str,
        uri: &str,
    ) -> Option<Response> {
        let window = web_sys::window().unwrap();
        let caches = window.caches().unwrap();

        let cache: Cache = JsFuture::from(caches.open(cache_name))
            .await
            .unwrap()
            .into();

        // Match the request URL to get the cached response
        let cached_response = JsFuture::from(cache.match_with_str(uri)).await.unwrap();

        if cached_response.is_null() || cached_response.is_undefined() {
            return None;
        }

        let cached_response: Response = cached_response.into();

        // Get the ETag header from the cached response
        let etag = cached_response.headers().get("etag").ok();

        if let Some(Some(etag)) = etag {
            request
                .headers()
                .set("If-None-Match", etag.as_str())
                .unwrap();

            Some(cached_response)
        } else {
            None
        }
    }

    pub(super) async fn save_response_to_cache(
        request: &Request,
        response: &Response,
        cache_name: &str,
    ) {
        let window = web_sys::window().unwrap();
        let caches = window.caches().unwrap();

        let cache: Cache = JsFuture::from(caches.open(cache_name))
            .await
            .unwrap()
            .into();

        JsFuture::from(cache.put_with_request(request, response))
            .await
            .unwrap();
    }
}
