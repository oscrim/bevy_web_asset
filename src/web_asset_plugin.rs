<<<<<<< HEAD
use bevy::prelude::*;
=======
#[cfg(not(target_arch = "wasm32"))]
use bevy::asset::FileAssetIo;
use bevy::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
>>>>>>> b097543 (Header resource)

use super::WebAssetIo;

/// Add this plugin to bevy to support loading http and https urls.
///
/// Needs to be added before Bevy's `DefaultPlugins`.
///
/// # Example
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use bevy_web_asset::WebAssetPlugin;
///
/// let mut app = App::new();
/// app.add_plugin(WebAssetPlugin);
/// app.add_plugins(DefaultPlugins);
/// ```
///});
#[derive(Default)]
pub struct WebAssetPlugin;

impl Plugin for WebAssetPlugin {
    fn build(&self, app: &mut App) {
        let http_headers = HttpHeader::default();
        let asset_io = WebAssetIo {
            default_io: AssetPlugin::default().create_platform_default_asset_io(),
            headers: http_headers.0.clone(),
        };

        app.insert_resource(AssetServer::new(asset_io));
        app.insert_resource(http_headers);
    }
}

#[derive(Default, Resource)]
pub struct HttpHeader(pub Arc<RwLock<String>>);
