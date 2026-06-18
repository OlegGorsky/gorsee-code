use gorsee_code_core::ModelCapability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelListResponse {
    pub object: Option<String>,
    pub data: Vec<ModelResponse>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResponse {
    pub id: String,
    pub object: Option<String>,
    pub created: Option<i64>,
    pub owned_by: Option<String>,
    pub metadata: Option<ModelMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub credit_multiplier: Option<f64>,
    pub context_window: Option<u64>,
    pub supports_streaming: Option<bool>,
    pub supports_tools: Option<bool>,
}

pub fn parse_models_response(response: ModelListResponse) -> Vec<ModelCapability> {
    response
        .data
        .into_iter()
        .map(|model| {
            let metadata = model.metadata.unwrap_or(ModelMetadata {
                credit_multiplier: None,
                context_window: None,
                supports_streaming: None,
                supports_tools: None,
            });
            ModelCapability {
                id: model.id,
                owned_by: model.owned_by,
                credit_multiplier: metadata.credit_multiplier.unwrap_or(1.0),
                supports_streaming: metadata.supports_streaming.unwrap_or(true),
                supports_tools: metadata.supports_tools.unwrap_or(false),
                context_window: metadata.context_window,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_credit_multiplier_from_metadata() {
        let response = serde_json::from_value::<ModelListResponse>(serde_json::json!({
            "object": "list",
            "data": [{
                "id": "neurogate/test",
                "object": "model",
                "owned_by": "torexxxproxy",
                "metadata": { "credit_multiplier": 2.5 }
            }]
        }))
        .unwrap();

        let models = parse_models_response(response);
        assert_eq!(models[0].id, "neurogate/test");
        assert_eq!(models[0].credit_multiplier, 2.5);
    }
}
