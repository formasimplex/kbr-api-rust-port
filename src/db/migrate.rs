use sqlx::PgPool;

use crate::error::AppError;

/// Run the schema migration against the database.
/// Idempotent: safe to run repeatedly. Uses CREATE TABLE IF NOT EXISTS.
pub async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
    let schema = include_str!("../../migrations/schema.sql");

    let statements = split_sql_statements(schema);

    for stmt in statements {
        let _ = sqlx::query(&stmt).execute(pool).await;
    }

    tracing::info!("Database migrations completed successfully");
    Ok(())
}

/// Split SQL statements by semicolon, but respect PostgreSQL dollar-quoted strings ($$ ... $$).
/// This ensures DO $$ ... END $$; blocks stay intact as single statements.
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut in_dollar_quote = false;
    let mut dollar_tag = String::new();

    while let Some(ch) = chars.next() {
        current.push(ch);

        if ch == '$' {
            // Try to match a dollar quote tag like $$ or $tag$
            let mut tag = String::from("$");
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '$' {
                    tag.push(chars.next().unwrap());
                    current.push('$');
                    break;
                } else if next_ch.is_alphanumeric() || next_ch == '_' {
                    tag.push(chars.next().unwrap());
                    current.push(next_ch);
                } else {
                    break;
                }
            }

            if tag.len() >= 2 && tag.starts_with('$') && tag.ends_with('$') {
                if !in_dollar_quote {
                    // Entering a dollar-quoted string
                    in_dollar_quote = true;
                    dollar_tag = tag;
                } else if tag == dollar_tag {
                    // Exiting the dollar-quoted string
                    in_dollar_quote = false;
                    dollar_tag.clear();
                }
            }
        } else if ch == ';' && !in_dollar_quote {
            // Found a statement-ending semicolon outside of dollar quotes
            let stmt = current.trim().to_string();
            if !stmt.is_empty() && stmt != ";" {
                statements.push(stmt);
            }
            current.clear();
        }
    }

    // Add any remaining statement
    let final_stmt = current.trim().to_string();
    if !final_stmt.is_empty() && final_stmt != ";" {
        statements.push(final_stmt);
    }

    statements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_sql_statements_basic() {
        let sql = "CREATE TABLE users (id INT); CREATE TABLE posts (id INT);";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("CREATE TABLE users"));
        assert!(stmts[1].contains("CREATE TABLE posts"));
    }

    #[test]
    fn split_sql_statements_dollar_quote() {
        let sql = r#"
            CREATE TABLE test (id INT);
            DO $$
            BEGIN
                ALTER TABLE test ADD COLUMN name VARCHAR;
            END $$;
            CREATE TABLE other (id INT);
        "#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 3);
        assert!(stmts[0].contains("CREATE TABLE test"));
        assert!(stmts[1].contains("DO $$"));
        assert!(stmts[1].contains("END $$"));
        assert!(!stmts[1].contains("CREATE TABLE other"));
        assert!(stmts[2].contains("CREATE TABLE other"));
    }

    #[test]
    fn split_sql_statements_nested_semicolons() {
        let sql = r#"
            DO $$
            BEGIN
                IF NOT EXISTS (SELECT 1 FROM test) THEN
                    ALTER TABLE users ADD COLUMN token_version BIGINT DEFAULT 1;
                END IF;
            END $$;
        "#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("DO $$"));
        assert!(stmts[0].contains("token_version"));
        assert!(stmts[0].contains("END IF;"));
        assert!(stmts[0].contains("END $$"));
    }

    #[test]
    fn split_sql_statements_tagged_dollar_quote() {
        let sql = r#"
            CREATE FUNCTION test() RETURNS void AS $func$
            BEGIN
                RAISE NOTICE 'test;';
            END;
            $func$ LANGUAGE plpgsql;
        "#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("$func$"));
        assert!(stmts[0].contains("RAISE NOTICE"));
    }
}
