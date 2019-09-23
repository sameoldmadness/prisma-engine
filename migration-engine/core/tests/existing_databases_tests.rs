#![allow(non_snake_case)]
#![allow(unused)]
mod test_harness;
use barrel::{types, Migration, SqlVariant};
use migration_core::api::GenericApi;
use pretty_assertions::{assert_eq, assert_ne};
use sql_migration_connector::SqlFamily;
use sql_migration_connector::{migration_database::MigrationDatabase, SqlMigrationConnector};
use sql_schema_describer::*;
use std::sync::Arc;
use test_harness::*;

#[test]
fn adding_a_model_for_an_existing_table_must_work() {
    test_each_backend(|api, barrel| {
        let initial_result = barrel.execute(|migration| {
            migration.create_table("Blog", |t| {
                t.add_column("id", types::primary());
            });
        });
        let dm = r#"
            model Blog {
                id Int @id
            }
        "#;
        let result = infer_and_apply(api, &dm).sql_schema;
        assert_eq!(initial_result, result);
    });
}

#[test]
fn bigint_columns_must_work() {
    // TODO: port when barrel supports arbitray primary keys
}

#[test]
fn removing_a_model_for_a_table_that_is_already_deleted_must_work() {
    test_each_backend(|api, barrel| {
        let dm1 = r#"
            model Blog {
                id Int @id
            }

            model Post {
                id Int @id
            }
        "#;
        let initial_result = infer_and_apply(api, &dm1).sql_schema;
        assert!(initial_result.has_table("Post"));

        let result = barrel.execute(|migration| {
            migration.drop_table("Post");
        });
        assert!(!result.has_table("Post"));

        let dm2 = r#"
            model Blog {
                id Int @id
            }
        "#;
        let final_result = infer_and_apply(api, &dm2).sql_schema;
        assert_eq!(result, final_result);
    });
}

#[test]
fn creating_a_field_for_an_existing_column_with_a_compatible_type_must_work() {
    test_each_backend(|api, barrel| {
        let initial_result = barrel.execute(|migration| {
            migration.create_table("Blog", |t| {
                t.add_column("id", types::primary());
                t.add_column("title", types::text());
            });
        });
        let dm = r#"
            model Blog {
                id Int @id
                title String
            }
        "#;
        let result = infer_and_apply(api, &dm).sql_schema;
        assert_eq!(initial_result, result);
    });
}

#[test]
fn creating_a_field_for_an_existing_column_and_changing_its_type_must_work() {
    test_each_backend(|api, barrel| {
        let initial_result = barrel.execute(|migration| {
            migration.create_table("Blog", |t| {
                t.add_column("id", types::primary());
                t.add_column("title", types::integer().nullable(true));
            });
        });
        let initial_column = initial_result.table_bang("Blog").column_bang("title");
        assert_eq!(initial_column.tpe.family, ColumnTypeFamily::Int);
        assert_eq!(initial_column.is_required(), false);

        let dm = r#"
            model Blog {
                id Int @id
                title String @unique
            }
        "#;
        let result = infer_and_apply(api, &dm).sql_schema;
        let table = result.table_bang("Blog");
        let column = table.column_bang("title");
        assert_eq!(column.tpe.family, ColumnTypeFamily::String);
        assert_eq!(column.is_required(), true);
        let index = table.indices.iter().find(|i| i.columns == vec!["title"]);
        assert_eq!(index.is_some(), true);
        assert_eq!(index.unwrap().tpe, IndexType::Unique);
    });
}

#[test]
fn creating_a_field_for_an_existing_column_and_simultaneously_making_it_optional() {
    test_each_backend(|api, barrel| {
        let initial_result = barrel.execute(|migration| {
            migration.create_table("Blog", |t| {
                t.add_column("id", types::primary());
                t.add_column("title", types::text());
            });
        });
        let initial_column = initial_result.table_bang("Blog").column_bang("title");
        assert_eq!(initial_column.is_required(), true);

        let dm = r#"
            model Blog {
                id Int @id
                title String?
            }
        "#;
        let result = infer_and_apply(api, &dm).sql_schema;
        let column = result.table_bang("Blog").column_bang("title");
        assert_eq!(column.is_required(), false);
    });
}

#[test]
fn creating_a_scalar_list_field_for_an_existing_table_must_work() {
    test_each_backend(|api, barrel| {
        let dm1 = r#"
            model Blog {
                id Int @id
            }
        "#;
        let initial_result = infer_and_apply(api, &dm1).sql_schema;
        assert!(!initial_result.has_table("Blog_tags"));

        let mut result = barrel.execute(|migration| {
            migration.create_table("Blog_tags", |t| {
                // TODO: barrel does not render this one correctly
                // TODO: the column should not be nullable. We just set it nullable because of our current inline relation nullability hack
                t.add_column("nodeId", types::foreign("Blog", "id").nullable(true));
                t.add_column("position", types::integer());
                t.add_column("value", types::text());
            });
        });
        // hacks for things i can't set in barrel due to limitations
        for table in &mut result.tables {
            if table.name == "Blog_tags" {
                for fk in &mut table.foreign_keys {
                    if fk.columns == vec!["nodeId".to_string()] {
                        fk.on_delete_action = ForeignKeyAction::Cascade
                    }
                }
                //                table.primary_key = Some(PrimaryKey {
                //                    columns: vec!["nodeId".to_string(), "position".to_string()],
                //                    sequence: None,
                //                });
            }
        }

        assert!(result.has_table("Blog_tags"));

        let dm2 = r#"
            model Blog {
                id Int @id
                tags String[]
            }
        "#;
        let mut final_result = infer_and_apply(api, &dm2).sql_schema;
        for table in &mut final_result.tables {
            if table.name == "Blog_tags" {
                // can't set that properly up again
                table.indices = vec![];
                table.primary_key = None;
            }
        }
        assert_eq!(result, final_result);
    });
}

#[test]
fn delete_a_field_for_a_non_existent_column_must_work() {
    test_each_backend(|api, barrel| {
        let dm1 = r#"
            model Blog {
                id Int @id
                title String
            }
        "#;
        let initial_result = infer_and_apply(api, &dm1).sql_schema;
        assert_eq!(initial_result.table_bang("Blog").column("title").is_some(), true);

        let result = barrel.execute(|migration| {
            // sqlite does not support dropping columns. So we are emulating it..
            migration.drop_table("Blog");
            migration.create_table("Blog", |t| {
                t.add_column("id", types::primary());
            });
        });
        assert_eq!(result.table_bang("Blog").column("title").is_some(), false);

        let dm2 = r#"
            model Blog {
                id Int @id
            }
        "#;
        let final_result = infer_and_apply(api, &dm2).sql_schema;
        assert_eq!(result, final_result);
    });
}

#[test]
fn deleting_a_scalar_list_field_for_a_non_existent_list_table_must_work() {
    test_each_backend(|api, barrel| {
        let dm1 = r#"
            model Blog {
                id Int @id
                tags String[]
            }
        "#;
        let initial_result = infer_and_apply(api, &dm1).sql_schema;
        assert!(initial_result.has_table("Blog_tags"));

        let result = barrel.execute(|migration| {
            migration.drop_table("Blog_tags");
        });
        assert!(!result.has_table("Blog_tags"));

        let dm2 = r#"
            model Blog {
                id Int @id
            }
        "#;
        let final_result = infer_and_apply(api, &dm2).sql_schema;
        assert_eq!(result, final_result);
    });
}

#[test]
fn updating_a_field_for_a_non_existent_column() {
    test_each_backend(|api, barrel| {
        let dm1 = r#"
            model Blog {
                id Int @id
                title String
            }
        "#;
        let initial_result = infer_and_apply(api, &dm1).sql_schema;
        let initial_column = initial_result.table_bang("Blog").column_bang("title");
        assert_eq!(initial_column.tpe.family, ColumnTypeFamily::String);

        let result = barrel.execute(|migration| {
            // sqlite does not support dropping columns. So we are emulating it..
            migration.drop_table("Blog");
            migration.create_table("Blog", |t| {
                t.add_column("id", types::primary());
            });
        });
        assert_eq!(result.table_bang("Blog").column("title").is_some(), false);

        let dm2 = r#"
            model Blog {
                id Int @id
                title Int @unique
            }
        "#;
        let final_result = infer_and_apply(api, &dm2).sql_schema;
        let final_column = final_result.table_bang("Blog").column_bang("title");
        assert_eq!(final_column.tpe.family, ColumnTypeFamily::Int);
        let index = final_result
            .table_bang("Blog")
            .indices
            .iter()
            .find(|i| i.columns == vec!["title"]);
        assert_eq!(index.is_some(), true);
        assert_eq!(index.unwrap().tpe, IndexType::Unique);
    });
}

#[test]
fn renaming_a_field_where_the_column_was_already_renamed_must_work() {
    test_each_backend(|api, barrel| {
        let dm1 = r#"
            model Blog {
                id Int @id
                title String
            }
        "#;
        let initial_result = infer_and_apply(api, &dm1).sql_schema;
        let initial_column = initial_result.table_bang("Blog").column_bang("title");
        assert_eq!(initial_column.tpe.family, ColumnTypeFamily::String);

        let result = barrel.execute(|migration| {
            // sqlite does not support renaming columns. So we are emulating it..
            migration.drop_table("Blog");
            migration.create_table("Blog", |t| {
                t.add_column("id", types::primary());
                t.add_column("new_title", types::text());
            });
        });
        assert_eq!(result.table_bang("Blog").column("new_title").is_some(), true);

        let dm2 = r#"
            model Blog {
                id Int @id
                title Float @map(name: "new_title")
            }
        "#;

        let final_result = infer_and_apply(api, &dm2).sql_schema;
        let final_column = final_result.table_bang("Blog").column_bang("new_title");

        assert_eq!(final_column.tpe.family, ColumnTypeFamily::Float);
        assert_eq!(final_result.table_bang("Blog").column("title").is_some(), false);
    })
}

fn test_each_backend<F>(test_fn: F)
where
    F: Fn(&dyn GenericApi, &BarrelMigrationExecutor) -> () + std::panic::RefUnwindSafe,
{
    test_each_backend_with_ignores(Vec::new(), test_fn);
}

fn test_each_backend_with_ignores<F>(ignores: Vec<SqlFamily>, test_fn: F)
where
    F: Fn(&dyn GenericApi, &BarrelMigrationExecutor) -> () + std::panic::RefUnwindSafe,
{
    // SQLite
    if !ignores.contains(&SqlFamily::Sqlite) {
        println!("Testing with SQLite now");
        let (inspector, database) = get_sqlite();

        println!("Running the test function now");
        let connector = SqlMigrationConnector::sqlite(&sqlite_test_file()).unwrap();
        let api = test_api(connector);

        let barrel_migration_executor = BarrelMigrationExecutor {
            inspector,
            database,
            sql_variant: SqlVariant::Sqlite,
        };

        test_fn(&api, &barrel_migration_executor);
    } else {
        println!("Ignoring SQLite")
    }
    // POSTGRES
    if !ignores.contains(&SqlFamily::Postgres) {
        println!("Testing with Postgres now");
        let (inspector, database) = get_postgres();

        println!("Running the test function now");
        let connector = SqlMigrationConnector::postgres(&postgres_url(), false).unwrap();
        let api = test_api(connector);

        let barrel_migration_executor = BarrelMigrationExecutor {
            inspector,
            database,
            sql_variant: SqlVariant::Pg,
        };

        test_fn(&api, &barrel_migration_executor);
    } else {
        println!("Ignoring Postgres")
    }
}

fn get_sqlite() -> (Arc<dyn SqlSchemaDescriberBackend>, Arc<dyn MigrationDatabase>) {
    let wrapper = database_wrapper(SqlFamily::Sqlite, &sqlite_test_file());
    let database = Arc::clone(&wrapper.database);

    let database_file_path = sqlite_test_file();
    let _ = std::fs::remove_file(database_file_path.clone()); // ignore potential errors

    let inspector = sql_schema_describer::sqlite::SqlSchemaDescriber::new(Arc::new(wrapper));

    (Arc::new(inspector), database)
}

fn get_postgres() -> (Arc<dyn SqlSchemaDescriberBackend>, Arc<dyn MigrationDatabase>) {
    let wrapper = database_wrapper(SqlFamily::Postgres, &postgres_url());
    let database = Arc::clone(&wrapper.database);

    let drop_schema = dbg!(format!("DROP SCHEMA IF EXISTS \"{}\" CASCADE;", SCHEMA_NAME));
    let _ = database.query_raw(SCHEMA_NAME, &drop_schema, &[]);

    let inspector = sql_schema_describer::postgres::SqlSchemaDescriber::new(Arc::new(wrapper));

    (Arc::new(inspector), database)
}

struct BarrelMigrationExecutor {
    inspector: Arc<dyn SqlSchemaDescriberBackend>,
    database: Arc<dyn MigrationDatabase>,
    sql_variant: barrel::backend::SqlVariant,
}

impl BarrelMigrationExecutor {
    fn execute<F>(&self, mut migrationFn: F) -> SqlSchema
    where
        F: FnMut(&mut Migration) -> (),
    {
        let mut migration = Migration::new().schema(SCHEMA_NAME);
        migrationFn(&mut migration);
        let full_sql = dbg!(migration.make_from(self.sql_variant));
        run_full_sql(&self.database, &full_sql);
        let mut result = self
            .inspector
            .describe(&SCHEMA_NAME.to_string())
            .expect("Introspection failed");
        // the presence of the _Migration table makes assertions harder. Therefore remove it.
        result.tables = result.tables.into_iter().filter(|t| t.name != "_Migration").collect();
        result
    }
}

fn run_full_sql(database: &Arc<dyn MigrationDatabase>, full_sql: &str) {
    for sql in full_sql.split(";") {
        if sql != "" {
            database.query_raw(SCHEMA_NAME, &sql, &[]).unwrap();
        }
    }
}
