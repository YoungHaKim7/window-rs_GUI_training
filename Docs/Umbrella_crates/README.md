An **umbrella crate** in Rust is a crate that acts as a central package or container for multiple related crates.

It is commonly used to organize a Rust workspace or provide a unified entry point to several related libraries.

### **Example**

Imagine a Rust project with this structure:

```bash
my_project/
├── Cargo.toml
├── umbrella/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── core/
│   ├── Cargo.toml
│   └── src/lib.rs
├── graphics/
│   ├── Cargo.toml
│   └── src/lib.rs
└── networking/
    ├── Cargo.toml
    └── src/lib.rs
```

The `umbrella` crate depends on the other crates:

```toml
# umbrella/Cargo.toml

[package]
name = "umbrella"
version = "0.1.0"
edition = "2024"

[dependencies]
core = { path = "../core" }
graphics = { path = "../graphics" }
networking = { path = "../networking" }
```

It can re-export their APIs:

```rs
// umbrella/src/lib.rs

pub use core::*;
pub use graphics::*;
pub use networking::*;
```

Now users can depend on just one crate:

```toml
[dependencies]
umbrella = { path = "../umbrella" }
```

### **Why use an umbrella crate?**

1. **Convenience:** Users install one crate instead of several.
2. **Unified API:** Related crates can be accessed through one dependency.
3. **Organization:** Separates functionality into smaller, independent crates.
4. **Compatibility:** The umbrella crate can coordinate versions of its component crates.

### **Umbrella crate vs. workspace**

| Concept        | Meaning                                                    |
| -------------- | ---------------------------------------------------------- |
| Workspace      | Organizes multiple Rust packages in one repository.        |
| Umbrella crate | Provides a unified dependency or API over multiple crates. |
| Normal crate   | A single Rust package containing a library or executable.  |

- **Important:** An umbrella crate is not a special Rust or Cargo feature. It is a software architecture pattern.

Think of it like this:

```
             umbrella crate
                   |
          +--------+--------+
          |        |        |
         core   graphics  networking
          |        |        |
       package  package  package
```

- In short, an umbrella crate is a **single entry point that brings multiple related Rust crates together**.
