use borsh::{BorshSchema, BorshSerialize};
mod schema;
use schema::PoolState;
use schema::PoolRequest;

fn main() -> std::io::Result<()> {
    let mut schema = PoolState::schema_container();
    for (name, def) in PoolRequest::schema_container().definitions.into_iter() {
        schema.definitions.insert(name, def);
    }
    schema.serialize(&mut std::io::stdout())
}
