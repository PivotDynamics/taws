use crate::aws::client::AwsClients;
use crate::aws::http::xml_to_json;
use anyhow::{anyhow, Result};
use tracing::debug;

/// Discover AWS regions available to the current account.
///
/// Uses EC2 DescribeRegions from the current AWS region and returns
/// only regions belonging to the same region family as the current region.
/// For example:
///   eu-west-1     -> eu-*
///   us-east-1     -> us-*
pub async fn discover_regions(clients: &AwsClients) -> Result<Vec<String>> {
    let response = clients
        .http
        .query_request("ec2", "DescribeRegions", &[("AllRegions", "true")])
        .await?;

    let json = xml_to_json(&response)?;

    let regions = json
        .pointer("/DescribeRegionsResponse/regionInfo/item")
        .ok_or_else(|| anyhow!("DescribeRegions response contains no regions"))?;

    let prefix = clients
        .region
        .split('-')
        .next()
        .ok_or_else(|| anyhow!("Invalid AWS region: {}", clients.region))?;

    let prefix = format!("{}-", prefix);

    let mut result = match regions {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                let region = item.get("regionName")?.as_str()?;
                let opt_in_status = item.get("optInStatus")?.as_str()?;
    
                if region.starts_with(&prefix) && matches!(opt_in_status, "opt-in-not-required" | "opted-in")
                {
                    Some(region.to_string())
                } else {
                    None
                }
            })
            .collect(),
    
        serde_json::Value::Object(item) => {
            let region = item.get("regionName").and_then(|v| v.as_str());
            let opt_in_status = item.get("optInStatus").and_then(|v| v.as_str());

            match (region, opt_in_status) {
                (Some(region), Some(status))
                if region.starts_with(&prefix)&& matches!(status, "opt-in-not-required" | "opted-in") =>
                {
                    vec![region.to_string()]
                }
                    _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    result.sort();
    result.dedup();

    if result.is_empty() {
        return Err(anyhow!(
            "AWS returned no regions matching '{}'",
            prefix
        ));
    }
    debug!("Discovered regions for {}: {:?}",
     clients.region,
     result
    );

    Ok(result)
}
