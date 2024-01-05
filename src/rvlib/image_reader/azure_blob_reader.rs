use std::{fs, vec};

use crate::{
    cache::ReadImageToCache,
    image_reader::core::SUPPORTED_EXTENSIONS,
    result::{to_rv, RvResult},
    rverr,
    types::ResultImage,
};
use azure_storage::ConnectionString;
use azure_storage_blobs::prelude::*;
use futures::StreamExt;
use lazy_static::lazy_static;
use tokio::runtime::Runtime;

lazy_static! {
    static ref RT: Runtime = Runtime::new().unwrap();
}

#[derive(Clone)]
pub struct AzureConnectionData {
    pub connection_string_path: String,
    pub container_name: String,
}

async fn blob_list(container_client: &ContainerClient, prefix: &str) -> RvResult<Vec<String>> {
    let mut res = vec![];
    let mut stream = container_client
        .list_blobs()
        .prefix(prefix.to_string())
        .into_stream();
    while let Some(value) = stream.next().await {
        let page = value.map_err(|e| {
            rverr!(
                "could not list blobs for container '{}' due to '{:?}'",
                container_client.container_name(),
                e
            )
        })?;
        for cont in page.blobs.blobs().filter(|b| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|ext| *ext == &b.name[(b.name.len() - ext.len())..b.name.len()])
        }) {
            res.push(cont.name.clone());
        }
        tracing::info!("retrieved {} blobs ", res.len());
    }
    Ok(res)
}

async fn download_blob(container_client: &ContainerClient, blob_name: &str) -> RvResult<Vec<u8>> {
    let blob_client = container_client.blob_client(blob_name);
    blob_client.get_content().await.map_err(to_rv)
}

#[derive(Clone)]
pub struct ReadImageFromAzureBlob {
    container_client: ContainerClient,
}

impl ReadImageToCache<AzureConnectionData> for ReadImageFromAzureBlob {
    fn new(conn_data: AzureConnectionData) -> RvResult<Self> {
        let connection_string =
            fs::read_to_string(&conn_data.connection_string_path).map_err(to_rv)?;
        let connection_string = ConnectionString::new(&connection_string).map_err(to_rv)?;
        let blob_service_client = BlobServiceClient::new(
            connection_string.account_name.unwrap(),
            connection_string.storage_credentials().map_err(to_rv)?,
        );
        let container_client = blob_service_client.container_client(conn_data.container_name);
        Ok(Self { container_client })
    }

    fn read(&self, blob_name: &str) -> ResultImage {
        let blob = RT.block_on(download_blob(&self.container_client, blob_name))?;
        image::load_from_memory(&blob).map_err(to_rv)
    }

    fn ls(&self, prefix: &str) -> RvResult<Vec<String>> {
        let res = RT.block_on(blob_list(&self.container_client, prefix));
        res
    }

    fn file_info(&self, _: &str) -> RvResult<String> {
        Err(rverr!("cannot read file info from azure blob",))
    }
}
