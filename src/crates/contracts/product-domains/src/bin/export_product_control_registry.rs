use bitfun_product_domains::product_control_owner_registry::owner_definitions;

fn main() {
    let output = serde_json::to_string_pretty(&owner_definitions())
        .expect("product-control owner registry must serialize");
    println!("{output}");
}
