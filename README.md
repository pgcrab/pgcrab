# Postgres Schema Documenter

This simple tool helps you generate a document that describes your Postgres schema.

**Features:**

- Customisable templating
  - with included template for: Markdown
- Show tables, with their comments, columns (and their types and comments), as well as indices and foreign key constraints.


## Usage

**Document a database:**

```
postgres_schema_documenter -C 'postgresql://user@host:port/dbname' doc --template basic_md_template.md.j2 > my_database_doc.md
```

Note: the connection string `postgresql:` is sufficient to use the libpq environment variables
as connection details.


**List items in the database that are missing comments:**

```
postgres_schema_documenter -C 'postgresql://user@host:port/dbname' list-uncommented
```


## Installation

### from source

`cargo install --path .` to install to your user,
or `cargo build --release` to just get a release build in your target directory.

You can use `cargo run -- <args>` to run from the source checkout.


### from binary

TODO


### as a Docker/Podman/OCI container

TODO
