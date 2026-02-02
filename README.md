# loc - Lines of Code Counter 

A fast lines of code counter written in Rust.

## Installation

```bash
cargo install loc_counter
```
## Usage

```bash
# Count LOC in current directory
loc

# Count with specific extensions
loc -e rs,py,js

# Output as JSON
loc --json

# Exclude directories
loc -x target,node_modules
```
