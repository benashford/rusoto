#![cfg(feature = "resourcegroupstaggingapi")]

extern crate rusoto_core;
extern crate rusoto_resourcegroupstaggingapi;

use rusoto_resourcegroupstaggingapi::{ResourceGroupsTaggingApi, ResourceGroupsTaggingApiClient, GetResourcesInput};
use rusoto_core::Region;

#[test]
fn should_get_resources() {
    let client = ResourceGroupsTaggingApiClient::simple(Region::UsEast1);
    let request = GetResourcesInput::default();

    let result = client.get_resources(request).sync().unwrap();
	println!("{:#?}", result);
}