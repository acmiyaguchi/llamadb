use sqlengine::{SqlEngine, ExecuteStatementResult};
use sqlsyntax::{ast, parse_statement};
use tempengine::TempEngine;

pub struct Connection {
    engine: TempEngine
}

impl Connection {
    pub fn new() -> Connection {
        Connection { engine: TempEngine::new() }
    }

    pub fn execute(&mut self, sql: &str) -> ExecuteStatementResult {
        let statement_ast = parse_statement(sql);
        self.engine.execute_statement(statement_ast)
    }

    pub fn execute_statement(&mut self, stmt: ast::Statement) -> ExecuteStatementResult {
        self.engine.execute_statement(stmt)
    }
}