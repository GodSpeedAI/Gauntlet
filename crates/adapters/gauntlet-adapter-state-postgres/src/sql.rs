//! A rusqlite-shaped shim over postgres::Client.
//!
//! Absorbs all type and API differences between rusqlite and postgres crate,
//! allowing state_store.rs to remain unchanged from the SQLite version.

use std::error::Error as StdError;
use std::fmt;
use std::sync::{Mutex, MutexGuard};

/// Error type matching rusqlite-style errors.
#[derive(Debug, Clone)]
pub enum Error {
    QueryReturnedNoRows,
    Postgres(String),
    CorruptData(String),
}

impl Error {
    fn from_postgres(err: postgres::Error) -> Self {
        // `postgres::Error`'s Display is the bare "db error" for server-side
        // failures; the server message lives on the DbError source.
        let detail = match err.as_db_error() {
            Some(db) => format!("{}: {}", db.code().code(), db.message()),
            None => match std::error::Error::source(&err) {
                Some(source) => format!("{}: {}", err, source),
                None => err.to_string(),
            },
        };
        Error::Postgres(detail)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::QueryReturnedNoRows => write!(f, "query returned no rows"),
            Error::Postgres(msg) => write!(f, "postgres error: {}", msg),
            Error::CorruptData(msg) => write!(f, "corrupt data: {}", msg),
        }
    }
}

impl StdError for Error {}

/// Trait for types that can be bound as SQL parameters.
pub trait SqlParam {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error>;
}

impl SqlParam for i64 {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(*self))
    }
}

impl SqlParam for u64 {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        let as_i64: i64 = (*self)
            .try_into()
            .map_err(|_| Error::CorruptData(format!("u64 value {} exceeds i64::MAX", self)))?;
        Ok(Box::new(as_i64))
    }
}

impl SqlParam for i32 {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(*self))
    }
}

impl SqlParam for u32 {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(*self as i64))
    }
}

impl SqlParam for usize {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        let as_i64: i64 = (*self)
            .try_into()
            .map_err(|_| Error::CorruptData(format!("usize value {} exceeds i64::MAX", self)))?;
        Ok(Box::new(as_i64))
    }
}

impl SqlParam for bool {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(*self))
    }
}

impl SqlParam for str {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(self.to_string()))
    }
}

impl SqlParam for String {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(self.clone()))
    }
}

impl<T: SqlParam + ?Sized> SqlParam for &T {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        T::to_pg(self)
    }
}

impl SqlParam for Vec<u8> {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(self.clone()))
    }
}

impl SqlParam for &[u8] {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        Ok(Box::new(self.to_vec()))
    }
}

impl<T: SqlParam> SqlParam for Option<T> {
    fn to_pg(&self) -> Result<Box<dyn postgres::types::ToSql + Sync>, Error> {
        match self {
            Some(val) => val.to_pg(),
            None => Ok(Box::new(None::<String>)),
        }
    }
}

/// Trait for extracting values from a row.
pub trait SqlRowValue: Sized {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error>;
}

impl SqlRowValue for i64 {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        row.try_get(idx).map_err(Error::from_postgres)
    }
}

impl SqlRowValue for u64 {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        let val: i64 = row.try_get(idx).map_err(Error::from_postgres)?;
        if val < 0 {
            Err(Error::CorruptData(format!(
                "BIGINT column contains negative value {}, expected u64",
                val
            )))
        } else {
            Ok(val as u64)
        }
    }
}

impl SqlRowValue for i32 {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        row.try_get(idx).map_err(Error::from_postgres)
    }
}

impl SqlRowValue for u32 {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        let val: i32 = row.try_get(idx).map_err(Error::from_postgres)?;
        if val < 0 {
            Err(Error::CorruptData(format!(
                "INT column contains negative value {}, expected u32",
                val
            )))
        } else {
            Ok(val as u32)
        }
    }
}

impl SqlRowValue for usize {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        let val: i64 = row.try_get(idx).map_err(Error::from_postgres)?;
        if val < 0 {
            Err(Error::CorruptData(format!(
                "BIGINT column contains negative value {}, expected usize",
                val
            )))
        } else {
            Ok(val as usize)
        }
    }
}

impl SqlRowValue for bool {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        row.try_get(idx).map_err(Error::from_postgres)
    }
}

impl SqlRowValue for String {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        row.try_get(idx).map_err(Error::from_postgres)
    }
}

// Note: We cannot directly implement SqlRowValue for &str since it requires 'static.
// Instead, callers should use String and borrow it as needed.

impl SqlRowValue for Vec<u8> {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        row.try_get(idx).map_err(Error::from_postgres)
    }
}

/// Accepts a column of any type and discards its value: used only to ask
/// whether the column is NULL, so `Option<T>` works for non-text columns.
struct NullProbe;

impl<'a> postgres::types::FromSql<'a> for NullProbe {
    fn from_sql(
        _ty: &postgres::types::Type,
        _raw: &'a [u8],
    ) -> Result<Self, Box<dyn StdError + Sync + Send>> {
        Ok(NullProbe)
    }

    fn accepts(_ty: &postgres::types::Type) -> bool {
        true
    }
}

impl<T: SqlRowValue> SqlRowValue for Option<T> {
    fn from_row(row: &postgres::Row, idx: usize) -> Result<Self, Error> {
        match row.try_get::<_, Option<NullProbe>>(idx) {
            Ok(None) => Ok(None),
            Ok(Some(_)) => T::from_row(row, idx).map(Some),
            Err(e) => Err(Error::from_postgres(e)),
        }
    }
}

/// Trait for indexing into rows.
pub trait RowIndex {
    fn index(&self) -> usize;
}

impl RowIndex for usize {
    fn index(&self) -> usize {
        *self
    }
}

impl RowIndex for u32 {
    fn index(&self) -> usize {
        *self as usize
    }
}

impl RowIndex for i32 {
    fn index(&self) -> usize {
        *self as usize
    }
}

impl RowIndex for u8 {
    fn index(&self) -> usize {
        *self as usize
    }
}

/// A row wrapper providing rusqlite-style get() method.
pub struct Row<'a>(&'a postgres::Row);

impl<'a> Row<'a> {
    pub fn new(row: &'a postgres::Row) -> Self {
        Row(row)
    }

    pub fn get<I: RowIndex, T: SqlRowValue>(&self, idx: I) -> Result<T, Error> {
        T::from_row(self.0, idx.index())
    }
}

/// Extension trait for optional results.
/// A prepared statement.
pub struct Statement {
    sql: String,
}

impl Statement {
    pub fn query_map<P: Params, T, F>(
        &self,
        conn: &Conn,
        params: P,
        mut f: F,
    ) -> Result<std::vec::IntoIter<Result<T, Error>>, Error>
    where
        F: FnMut(&Row) -> Result<T, Error>,
    {
        let rewritten = rewrite_placeholders(&self.sql);
        let owned: Vec<Box<dyn postgres::types::ToSql + Sync>> = params.to_postgres_params()?;
        let refs: Vec<&(dyn postgres::types::ToSql + Sync)> = owned.iter().map(|b| &**b).collect();

        let rows = conn
            .client_mut()
            .query(&rewritten, &refs)
            .map_err(Error::from_postgres)?;

        let results: Vec<Result<T, Error>> = rows
            .iter()
            .map(|pg_row| {
                let row = Row::new(pg_row);
                f(&row)
            })
            .collect();

        Ok(results.into_iter())
    }
}

/// Connection wrapper using a Mutex for interior mutability (Send + Sync).
pub struct Conn {
    client: Mutex<postgres::Client>,
}

impl Conn {
    pub fn new(client: postgres::Client) -> Self {
        Conn {
            client: Mutex::new(client),
        }
    }

    pub fn client_mut(&self) -> MutexGuard<'_, postgres::Client> {
        self.client
            .lock()
            .expect("the store connection is not poisoned")
    }

    pub fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize, Error> {
        let owned: Vec<Box<dyn postgres::types::ToSql + Sync>> = params.to_postgres_params()?;
        let refs: Vec<&(dyn postgres::types::ToSql + Sync)> = owned.iter().map(|b| &**b).collect();

        let rewritten = rewrite_placeholders(sql);
        let res = self
            .client_mut()
            .execute(&rewritten, &refs)
            .map_err(Error::from_postgres)?;

        usize::try_from(res)
            .map_err(|_| Error::CorruptData("execute result exceeds usize::MAX".to_string()))
    }

    pub fn query_row<P: Params, T, F>(&self, sql: &str, params: P, f: F) -> Result<T, Error>
    where
        F: FnOnce(&Row) -> Result<T, Error>,
    {
        let owned: Vec<Box<dyn postgres::types::ToSql + Sync>> = params.to_postgres_params()?;
        let refs: Vec<&(dyn postgres::types::ToSql + Sync)> = owned.iter().map(|b| &**b).collect();

        let rewritten = rewrite_placeholders(sql);
        let rows = self
            .client_mut()
            .query(&rewritten, &refs)
            .map_err(Error::from_postgres)?;

        match rows.len() {
            0 => Err(Error::QueryReturnedNoRows),
            // rusqlite `query_row` semantics: the first row is used and any
            // further rows are ignored.
            _ => {
                let row = Row::new(&rows[0]);
                f(&row)
            }
        }
    }

    pub fn prepare(&self, sql: &str) -> Result<Statement, Error> {
        Ok(Statement {
            sql: sql.to_string(),
        })
    }

    pub fn last_insert_rowid(&self) -> i64 {
        0
    }
}

/// Trait for converting parameter tuples to postgres-compatible form.
pub trait Params {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error>;
}

impl Params for () {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![])
    }
}

// Tuple implementations for 1-12 parameters
impl<A: SqlParam> Params for (A,) {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![self.0.to_pg()?])
    }
}

impl<A: SqlParam, B: SqlParam> Params for (A, B) {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![self.0.to_pg()?, self.1.to_pg()?])
    }
}

impl<A: SqlParam, B: SqlParam, C: SqlParam> Params for (A, B, C) {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![self.0.to_pg()?, self.1.to_pg()?, self.2.to_pg()?])
    }
}

impl<A: SqlParam, B: SqlParam, C: SqlParam, D: SqlParam> Params for (A, B, C, D) {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
        ])
    }
}

impl<A: SqlParam, B: SqlParam, C: SqlParam, D: SqlParam, E: SqlParam> Params for (A, B, C, D, E) {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
        ])
    }
}

impl<A: SqlParam, B: SqlParam, C: SqlParam, D: SqlParam, E: SqlParam, F: SqlParam> Params
    for (A, B, C, D, E, F)
{
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
            self.5.to_pg()?,
        ])
    }
}

impl<A: SqlParam, B: SqlParam, C: SqlParam, D: SqlParam, E: SqlParam, F: SqlParam, G: SqlParam>
    Params for (A, B, C, D, E, F, G)
{
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
            self.5.to_pg()?,
            self.6.to_pg()?,
        ])
    }
}

impl<
    A: SqlParam,
    B: SqlParam,
    C: SqlParam,
    D: SqlParam,
    E: SqlParam,
    F: SqlParam,
    G: SqlParam,
    H: SqlParam,
> Params for (A, B, C, D, E, F, G, H)
{
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
            self.5.to_pg()?,
            self.6.to_pg()?,
            self.7.to_pg()?,
        ])
    }
}

impl<
    A: SqlParam,
    B: SqlParam,
    C: SqlParam,
    D: SqlParam,
    E: SqlParam,
    F: SqlParam,
    G: SqlParam,
    H: SqlParam,
    I: SqlParam,
> Params for (A, B, C, D, E, F, G, H, I)
{
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
            self.5.to_pg()?,
            self.6.to_pg()?,
            self.7.to_pg()?,
            self.8.to_pg()?,
        ])
    }
}

impl<
    A: SqlParam,
    B: SqlParam,
    C: SqlParam,
    D: SqlParam,
    E: SqlParam,
    F: SqlParam,
    G: SqlParam,
    H: SqlParam,
    I: SqlParam,
    J: SqlParam,
> Params for (A, B, C, D, E, F, G, H, I, J)
{
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
            self.5.to_pg()?,
            self.6.to_pg()?,
            self.7.to_pg()?,
            self.8.to_pg()?,
            self.9.to_pg()?,
        ])
    }
}

impl<
    A: SqlParam,
    B: SqlParam,
    C: SqlParam,
    D: SqlParam,
    E: SqlParam,
    F: SqlParam,
    G: SqlParam,
    H: SqlParam,
    I: SqlParam,
    J: SqlParam,
    K: SqlParam,
> Params for (A, B, C, D, E, F, G, H, I, J, K)
{
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
            self.5.to_pg()?,
            self.6.to_pg()?,
            self.7.to_pg()?,
            self.8.to_pg()?,
            self.9.to_pg()?,
            self.10.to_pg()?,
        ])
    }
}

impl<
    A: SqlParam,
    B: SqlParam,
    C: SqlParam,
    D: SqlParam,
    E: SqlParam,
    F: SqlParam,
    G: SqlParam,
    H: SqlParam,
    I: SqlParam,
    J: SqlParam,
    K: SqlParam,
    L: SqlParam,
> Params for (A, B, C, D, E, F, G, H, I, J, K, L)
{
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![
            self.0.to_pg()?,
            self.1.to_pg()?,
            self.2.to_pg()?,
            self.3.to_pg()?,
            self.4.to_pg()?,
            self.5.to_pg()?,
            self.6.to_pg()?,
            self.7.to_pg()?,
            self.8.to_pg()?,
            self.9.to_pg()?,
            self.10.to_pg()?,
            self.11.to_pg()?,
        ])
    }
}

// Array support
impl<T: SqlParam> Params for [T; 0] {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![])
    }
}

impl<T: SqlParam> Params for [T; 1] {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![self[0].to_pg()?])
    }
}

impl<T: SqlParam> Params for [T; 2] {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        Ok(vec![self[0].to_pg()?, self[1].to_pg()?])
    }
}

// Slice and Vec support
impl<T: SqlParam> Params for &[T] {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        self.iter().map(|p| p.to_pg()).collect()
    }
}

impl<T: SqlParam> Params for Vec<T> {
    fn to_postgres_params(&self) -> Result<Vec<Box<dyn postgres::types::ToSql + Sync>>, Error> {
        self.iter().map(|p| p.to_pg()).collect()
    }
}

fn rewrite_placeholders(sql: &str) -> String {
    let mut result = String::new();
    let mut chars = sql.chars().peekable();
    let mut auto_index = 1;

    while let Some(ch) = chars.next() {
        if ch == '?' {
            if let Some(&next) = chars.peek() {
                if next.is_ascii_digit() {
                    chars.next();
                    let mut num_str = next.to_string();
                    while let Some(&digit_char) = chars.peek() {
                        if digit_char.is_ascii_digit() {
                            chars.next();
                            num_str.push(digit_char);
                        } else {
                            break;
                        }
                    }
                    result.push('$');
                    result.push_str(&num_str);
                } else {
                    result.push('$');
                    result.push_str(&auto_index.to_string());
                    auto_index += 1;
                }
            } else {
                result.push('$');
                result.push_str(&auto_index.to_string());
                auto_index += 1;
            }
        } else {
            result.push(ch);
        }
    }

    result
}
