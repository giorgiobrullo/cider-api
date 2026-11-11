// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Apple Music catalog endpoints of the v2 API.

use reqwest::Method;
use serde_json::Value;

use super::library::encode;
use super::{fetch, CatalogOptions, SearchOptions, REMOTE_TIMEOUT, V2};
use crate::client::CiderError;

/// Apple Music API proxy. Created by [`V2::catalog`].
///
/// Cider adds the user's storefront and credentials on its side and relays
/// Apple's response unchanged, so results are raw JSON in
/// [Apple Music API](https://developer.apple.com/documentation/applemusicapi)
/// format. All three methods are read-only.
///
/// # Errors
///
/// When Apple rejects a request, these methods return
/// [`CiderError::Server`] with code `AMAPI_ERROR` and Apple's HTTP status.
#[derive(Debug, Clone, Copy)]
pub struct Catalog<'a> {
    pub(super) v2: V2<'a>,
}

impl Catalog<'_> {
    /// Search the Apple Music catalog.
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), cider_api::CiderError> {
    /// use cider_api::{v2::SearchOptions, CiderClient};
    ///
    /// let client = CiderClient::new();
    /// let options = SearchOptions {
    ///     types: vec!["songs".into()],
    ///     limit: Some(5),
    ///     ..Default::default()
    /// };
    /// let results = client.v2().catalog().search("flume", &options).await?;
    /// for song in results["results"]["songs"]["data"].as_array().into_iter().flatten() {
    ///     println!("{}", song["attributes"]["name"]);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn search(&self, term: &str, options: &SearchOptions) -> Result<Value, CiderError> {
        let mut query: Vec<(&str, String)> = vec![("term", term.to_string())];
        if !options.types.is_empty() {
            query.push(("types", options.types.join(",")));
        }
        if let Some(limit) = options.limit {
            query.push(("limit", limit.clamp(1, 25).to_string()));
        }
        if let Some(offset) = options.offset {
            query.push(("offset", offset.to_string()));
        }
        if let Some(language) = &options.language {
            query.push(("l", language.clone()));
        }
        if let Some(storefront) = &options.storefront {
            query.push(("storefront", storefront.clone()));
        }
        fetch(
            self.v2
                .req(Method::GET, "/amapi/search")
                .timeout(REMOTE_TIMEOUT)
                .query(&query),
        )
        .await
    }

    /// Look up one catalog resource.
    ///
    /// `resource_type` is one of `songs`, `albums`, `artists`, `playlists`,
    /// `music-videos`, `stations`, `curators`, `apple-curators` or
    /// `record-labels`.
    ///
    /// # Errors
    ///
    /// See the [type-level docs](Self#errors).
    pub async fn get(
        &self,
        resource_type: &str,
        id: &str,
        options: &CatalogOptions,
    ) -> Result<Value, CiderError> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if !options.include.is_empty() {
            query.push(("include", options.include.join(",")));
        }
        if !options.views.is_empty() {
            query.push(("views", options.views.join(",")));
        }
        if let Some(language) = &options.language {
            query.push(("l", language.clone()));
        }
        let path = format!("/amapi/catalog/{}/{}", encode(resource_type), encode(id));
        fetch(
            self.v2
                .req(Method::GET, &path)
                .timeout(REMOTE_TIMEOUT)
                .query(&query),
        )
        .await
    }

    /// Send any read-only Apple Music API request.
    ///
    /// `path` is a relative Apple Music API path starting with `v1/`, with an
    /// optional query string, e.g. `"v1/me/library/playlists?limit=5"`. A
    /// leading `/` is accepted. The literal `$STOREFRONT` is replaced with
    /// the user's storefront.
    ///
    /// # Errors
    ///
    /// Returns [`CiderError::Server`] with code `INVALID_REQUEST` for paths
    /// outside `v1/`. See also the [type-level docs](Self#errors).
    pub async fn raw(&self, path: &str) -> Result<Value, CiderError> {
        let path = format!("/amapi/raw/{}", path.trim_start_matches('/'));
        fetch(self.v2.req(Method::GET, &path).timeout(REMOTE_TIMEOUT)).await
    }
}
