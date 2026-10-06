// SPDX-License-Identifier: Apache-2.0

//! Writes the frontend IPC bindings generated from the command surface.
//! Run with `pnpm bindings`.

use std::path::Path;

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(scoplen_client_app::BINDINGS_PATH);
    scoplen_client_app::write_bindings(&path).expect("bindings written");
    println!("wrote {}", path.display());
}
