// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Plugin endpoints of the v2 API.

use std::time::Duration;

use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::{fetch, V2};
use crate::client::CiderError;

/// Cider waits up to three minutes for the user to answer the install prompt.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(190);

/// Plugin endpoints. Created by [`V2::plugins`].
#[derive(Debug, Clone, Copy)]
pub struct Plugins<'a> {
    pub(super) v2: V2<'a>,
}

/// A plugin that was just installed
/// ([`Plugins::install`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    /// Plugin identifier from its manifest (e.g. `"com.example.plugin"`).
    #[serde(default)]
    pub identifier: String,

    /// Display name.
    #[serde(default)]
    pub name: String,

    /// Version from the manifest.
    #[serde(default)]
    pub version: String,

    /// Directory the plugin was installed into, on the machine running Cider.
    #[serde(default)]
    pub install_path: String,
}

#[derive(Debug, Serialize)]
struct InstallBody<'a> {
    url: &'a str,
}

impl Plugins<'_> {
    /// Install a Cider plugin from a `.zip` URL.
    ///
    /// Cider downloads the archive, validates its `plugin.yml` and shows a
    /// consent prompt with the manifest; this call resolves once the user
    /// answers. `url` must be `http://` or `https://`, at most 2048
    /// characters.
    ///
    /// Sends `POST /api/v2/plugins/install`.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with one of these codes:
    ///
    /// - `PLUGIN_INSTALL_DENIED` — the user declined.
    /// - `PLUGIN_INSTALL_TIMEOUT` — the user did not answer in time.
    /// - `PLUGIN_INSTALL_BUSY` — another install prompt is already open.
    /// - `PLUGIN_INSTALL_COOLDOWN` — a request was made less than 2 seconds
    ///   ago.
    /// - `PLUGIN_DOWNLOAD_FAILED` — Cider could not fetch `url`.
    /// - `PLUGIN_BAD_ZIP`, `PLUGIN_BAD_MANIFEST` — the archive is not a valid
    ///   plugin.
    /// - `INVALID_REQUEST` — `url` is malformed.
    ///
    /// Returns [`CiderError::Interrupted`] if the user takes longer than
    /// about 30 seconds on Cider 4.0, which closes the connection before its
    /// own prompt times out. The install can still complete if the user
    /// approves afterwards.
    pub async fn install(&self, url: &str) -> Result<InstalledPlugin, CiderError> {
        fetch(
            self.v2
                .req(Method::POST, "/plugins/install")
                .timeout(INSTALL_TIMEOUT)
                .json(&InstallBody { url }),
        )
        .await
    }
}
