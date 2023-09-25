use actix_web::web::{self, Data};
use actix_web::{HttpRequest, HttpResponse};
use deserr::actix_web::AwebJson;
use deserr::Deserr;
use index_scheduler::IndexScheduler;
use log::debug;
use meilisearch_types::deserr::DeserrJsonError;
use meilisearch_types::error::ResponseError;
use meilisearch_types::keys::actions;
use serde_json::json;

use crate::analytics::Analytics;
use crate::extractors::authentication::policies::ActionPolicy;
use crate::extractors::authentication::GuardedData;
use crate::extractors::sequential_extractor::SeqHandler;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("")
            .route(web::get().to(SeqHandler(get_features)))
            .route(web::patch().to(SeqHandler(patch_features))),
    );
}

async fn get_features(
    index_scheduler: GuardedData<
        ActionPolicy<{ actions::EXPERIMENTAL_FEATURES_GET }>,
        Data<IndexScheduler>,
    >,
    req: HttpRequest,
    analytics: Data<dyn Analytics>,
) -> Result<HttpResponse, ResponseError> {
    let features = index_scheduler.features()?;

    analytics.publish("Experimental features Seen".to_string(), json!(null), Some(&req));
    debug!("returns: {:?}", features.runtime_features());
    Ok(HttpResponse::Ok().json(features.runtime_features()))
}

#[derive(Debug, Deserr)]
#[deserr(error = DeserrJsonError, rename_all = camelCase, deny_unknown_fields)]
pub struct RuntimeTogglableFeatures {
    #[deserr(default)]
    pub score_details: Option<bool>,
    #[deserr(default)]
    pub vector_store: Option<bool>,
    #[deserr(default)]
    pub export_puffin_reports: Option<bool>,
}

async fn patch_features(
    index_scheduler: GuardedData<
        ActionPolicy<{ actions::EXPERIMENTAL_FEATURES_UPDATE }>,
        Data<IndexScheduler>,
    >,
    new_features: AwebJson<RuntimeTogglableFeatures, DeserrJsonError>,
    req: HttpRequest,
    analytics: Data<dyn Analytics>,
) -> Result<HttpResponse, ResponseError> {
    let features = index_scheduler.features()?;

    let old_features = features.runtime_features();
    let new_features = meilisearch_types::features::RuntimeTogglableFeatures {
        score_details: new_features.0.score_details.unwrap_or(old_features.score_details),
        vector_store: new_features.0.vector_store.unwrap_or(old_features.vector_store),
        export_puffin_reports: new_features
            .0
            .export_puffin_reports
            .unwrap_or(old_features.export_puffin_reports),
    };

    // explicitly destructure for analytics rather than using the `Serialize` implementation, because
    // the it renames to camelCase, which we don't want for analytics.
    // **Do not** ignore fields with `..` or `_` here, because we want to add them in the future.
    let meilisearch_types::features::RuntimeTogglableFeatures {
        score_details,
        vector_store,
        export_puffin_reports,
    } = new_features;

    analytics.publish(
        "Experimental features Updated".to_string(),
        json!({
            "score_details": score_details,
            "vector_store": vector_store,
            "export_puffin_reports": export_puffin_reports,
        }),
        Some(&req),
    );
    index_scheduler.put_runtime_features(new_features)?;
    Ok(HttpResponse::Ok().json(new_features))
}
