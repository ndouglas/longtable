//! Query compilation and execution for Longtable.
//!
//! This module provides:
//! - [`CompiledQuery`] - A compiled query ready for execution
//! - [`QueryCompiler`] - Compiles `QueryDecl` into `CompiledQuery`
//! - [`QueryExecutor`] - Executes queries against a World

use std::cmp::Ordering;

use longtable_foundation::{Interner, LtVec, Result, Value};
use longtable_language::declaration::{OrderDirection, QueryDecl};
use longtable_language::{Ast, CompiledExpr, Vm, compile_expression};
use longtable_storage::World;

use crate::pattern::{Bindings, CompiledPattern, PatternCompiler, PatternMatcher};

// =============================================================================
// Compiled Query Types
// =============================================================================

/// A compiled query ready for execution.
#[derive(Clone, Debug)]
pub struct CompiledQuery {
    /// Compiled pattern for matching
    pub pattern: CompiledPattern,
    /// Local bindings (variable name, expression to compute value)
    pub bindings: Vec<(String, CompiledExpr)>,
    /// Aggregation expressions (variable name, expression for aggregate)
    pub aggregates: Vec<(String, CompiledExpr)>,
    /// Variables to group by
    pub group_by: Vec<String>,
    /// Guard conditions (all must evaluate to true)
    pub guards: Vec<CompiledExpr>,
    /// Sort order (variable name, direction)
    pub order_by: Vec<(String, OrderDirection)>,
    /// Maximum results to return
    pub limit: Option<usize>,
    /// Expression to evaluate for each result
    pub return_expr: Option<CompiledExpr>,
    /// Variable names in order (for binding lookup)
    pub binding_vars: Vec<String>,
}

// =============================================================================
// Query Compiler
// =============================================================================

/// Compiles query declarations into executable queries.
pub struct QueryCompiler;

impl QueryCompiler {
    /// Compile a query declaration into a compiled query.
    ///
    /// # Errors
    ///
    /// Returns an error if compilation fails.
    pub fn compile(query: &QueryDecl, interner: &mut Interner) -> Result<CompiledQuery> {
        // Compile pattern
        let pattern = PatternCompiler::compile(&query.pattern, interner)?;

        // Collect all variable names for binding lookup
        let mut binding_vars = Vec::new();
        for clause in &pattern.clauses {
            if !binding_vars.contains(&clause.entity_var) {
                binding_vars.push(clause.entity_var.clone());
            }
            if let crate::pattern::CompiledBinding::Variable(v) = &clause.binding {
                if !binding_vars.contains(v) {
                    binding_vars.push(v.clone());
                }
            }
        }

        // Compile let bindings
        let mut compiled_bindings = Vec::new();
        for (name, ast) in &query.bindings {
            let bytecode = Self::compile_expr_with_vars(ast, &binding_vars)?;
            compiled_bindings.push((name.clone(), bytecode));
            if !binding_vars.contains(name) {
                binding_vars.push(name.clone());
            }
        }

        // Compile aggregates
        let mut compiled_aggregates = Vec::new();
        for (name, ast) in &query.aggregates {
            let bytecode = Self::compile_expr_with_vars(ast, &binding_vars)?;
            compiled_aggregates.push((name.clone(), bytecode));
        }

        // Compile guards
        let mut compiled_guards = Vec::new();
        for ast in &query.guards {
            let bytecode = Self::compile_expr_with_vars(ast, &binding_vars)?;
            compiled_guards.push(bytecode);
        }

        // Compile return expression
        let return_expr = if let Some(ast) = &query.return_expr {
            Some(Self::compile_expr_with_vars(ast, &binding_vars)?)
        } else {
            None
        };

        Ok(CompiledQuery {
            pattern,
            bindings: compiled_bindings,
            aggregates: compiled_aggregates,
            group_by: query.group_by.clone(),
            guards: compiled_guards,
            order_by: query.order_by.clone(),
            limit: query.limit,
            return_expr,
            binding_vars,
        })
    }

    /// Compile an expression with variable references resolved to binding indices.
    fn compile_expr_with_vars(ast: &Ast, vars: &[String]) -> Result<CompiledExpr> {
        // Use the standalone expression compiler with variable context
        compile_expression(ast, vars)
    }
}

// =============================================================================
// Query Executor
// =============================================================================

/// Executes compiled queries against a world.
pub struct QueryExecutor;

impl QueryExecutor {
    /// Execute a query and return all results.
    ///
    /// # Errors
    ///
    /// Returns an error if query execution fails.
    pub fn execute(query: &CompiledQuery, world: &World) -> Result<Vec<Value>> {
        // Step 1: Pattern matching - get all binding sets
        let all_bindings = PatternMatcher::match_pattern(&query.pattern, world);

        if all_bindings.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: Apply let bindings and filter by guards
        let mut filtered_bindings = Vec::new();
        for bindings in all_bindings {
            // Convert Bindings to value vector for VM
            let mut values = Self::bindings_to_vec(&bindings, &query.binding_vars);

            // Apply let bindings
            for (name, expr) in &query.bindings {
                let mut vm = Vm::new();
                vm.set_bindings(values.clone());
                let result = vm.execute_bytecode(&expr.code, &expr.constants)?;

                // Add to values
                let idx = query
                    .binding_vars
                    .iter()
                    .position(|v| v == name)
                    .unwrap_or(values.len());
                if idx < values.len() {
                    values[idx] = result;
                } else {
                    values.push(result);
                }
            }

            // Check guards
            let mut pass = true;
            for guard in &query.guards {
                let mut vm = Vm::new();
                vm.set_bindings(values.clone());
                let result = vm.execute_bytecode(&guard.code, &guard.constants)?;
                if result != Value::Bool(true) {
                    pass = false;
                    break;
                }
            }

            if pass {
                filtered_bindings.push((bindings, values));
            }
        }

        // Step 3: Group by (if specified)
        let grouped = if query.group_by.is_empty() {
            // No grouping - treat all as one group
            vec![filtered_bindings]
        } else {
            Self::group_bindings(&filtered_bindings, &query.group_by, &query.binding_vars)
        };

        // Step 4: Apply aggregations within each group
        let aggregated = Self::apply_aggregations(&grouped, query);

        // Step 5: Order by
        let mut ordered = aggregated;
        if !query.order_by.is_empty() {
            Self::sort_results(&mut ordered, &query.order_by, &query.binding_vars);
        }

        // Step 6: Apply limit
        if let Some(limit) = query.limit {
            ordered.truncate(limit);
        }

        // Step 7: Evaluate return expression for each result
        let results = if let Some(return_expr) = &query.return_expr {
            let mut output = Vec::new();
            for values in ordered {
                let mut vm = Vm::new();
                vm.set_bindings(values);
                let result = vm.execute_bytecode(&return_expr.code, &return_expr.constants)?;
                output.push(result);
            }
            output
        } else {
            // Return first bound entity if no return expression
            ordered
                .into_iter()
                .filter_map(|v| v.first().cloned())
                .collect()
        };

        Ok(results)
    }

    /// Execute query and return first result only.
    ///
    /// # Errors
    ///
    /// Returns an error if query execution fails.
    pub fn execute_one(query: &CompiledQuery, world: &World) -> Result<Option<Value>> {
        // Use limit 1 optimization
        let mut limited = query.clone();
        limited.limit = Some(1);
        let results = Self::execute(&limited, world)?;
        Ok(results.into_iter().next())
    }

    /// Check if any results exist (early exit).
    ///
    /// # Errors
    ///
    /// Returns an error if query execution fails.
    pub fn exists(query: &CompiledQuery, world: &World) -> Result<bool> {
        let result = Self::execute_one(query, world)?;
        Ok(result.is_some())
    }

    /// Count results without full materialization.
    ///
    /// # Errors
    ///
    /// Returns an error if query execution fails.
    pub fn count(query: &CompiledQuery, world: &World) -> Result<usize> {
        // For now, full execute and count
        // TODO: Optimize to avoid materializing return values
        let results = Self::execute(query, world)?;
        Ok(results.len())
    }

    // -------------------------------------------------------------------------
    // Helper methods
    // -------------------------------------------------------------------------

    fn bindings_to_vec(bindings: &Bindings, vars: &[String]) -> Vec<Value> {
        vars.iter()
            .map(|v| bindings.get(v).cloned().unwrap_or(Value::Nil))
            .collect()
    }

    #[allow(clippy::mutable_key_type)]
    fn group_bindings(
        bindings: &[(Bindings, Vec<Value>)],
        group_by: &[String],
        vars: &[String],
    ) -> Vec<Vec<(Bindings, Vec<Value>)>> {
        use std::collections::HashMap;

        // Using Vec<Value> as key is intentional for grouping semantics.
        // The values used as keys are not mutated during grouping.
        let mut groups: HashMap<Vec<Value>, Vec<(Bindings, Vec<Value>)>> = HashMap::new();

        for (b, values) in bindings {
            // Extract group key
            let key: Vec<Value> = group_by
                .iter()
                .map(|var| {
                    vars.iter()
                        .position(|v| v == var)
                        .and_then(|i| values.get(i).cloned())
                        .unwrap_or(Value::Nil)
                })
                .collect();

            groups
                .entry(key)
                .or_default()
                .push((b.clone(), values.clone()));
        }

        groups.into_values().collect()
    }

    fn apply_aggregations(
        groups: &[Vec<(Bindings, Vec<Value>)>],
        query: &CompiledQuery,
    ) -> Vec<Vec<Value>> {
        if query.aggregates.is_empty() {
            // No aggregation - flatten groups
            return groups
                .iter()
                .flat_map(|g| g.iter().map(|(_, v)| v.clone()))
                .collect();
        }

        // Apply aggregations to each group
        let mut results = Vec::new();
        for group in groups {
            if group.is_empty() {
                continue;
            }

            // Start with first row's values as base
            let mut aggregated = group[0].1.clone();

            // Compute each aggregate
            for (name, expr) in &query.aggregates {
                // Collect values to aggregate from all rows in group
                let collected: Vec<Value> = group
                    .iter()
                    .map(|(_, v)| {
                        // Evaluate aggregate expression for this row
                        let mut vm = Vm::new();
                        vm.set_bindings(v.clone());
                        vm.execute_bytecode(&expr.code, &expr.constants)
                            .unwrap_or(Value::Nil)
                    })
                    .collect();

                // The aggregate result is the collected vector
                // (actual aggregation like sum/count happens in the expression)
                let agg_result = Value::Vec(LtVec::from_iter(collected));

                // Find or add the aggregate variable
                if let Some(idx) = query.binding_vars.iter().position(|v| v == name) {
                    if idx < aggregated.len() {
                        aggregated[idx] = agg_result;
                    }
                }
            }

            results.push(aggregated);
        }

        results
    }

    fn sort_results(
        results: &mut [Vec<Value>],
        order_by: &[(String, OrderDirection)],
        vars: &[String],
    ) {
        results.sort_by(|a, b| {
            for (var, direction) in order_by {
                let idx = vars.iter().position(|v| v == var);
                let cmp = match idx {
                    Some(i) => Self::compare_values(a.get(i), b.get(i)),
                    None => Ordering::Equal,
                };

                if cmp != Ordering::Equal {
                    return match direction {
                        OrderDirection::Asc => cmp,
                        OrderDirection::Desc => cmp.reverse(),
                    };
                }
            }
            Ordering::Equal
        });
    }

    fn compare_values(a: Option<&Value>, b: Option<&Value>) -> Ordering {
        match (a, b) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(va), Some(vb)) => va.partial_cmp(vb).unwrap_or(Ordering::Equal),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use longtable_foundation::LtMap;
    use longtable_language::Span;
    use longtable_language::declaration::{Pattern, PatternClause, PatternValue};
    use longtable_storage::ComponentSchema;

    fn setup_world() -> World {
        let mut world = World::new(42);

        // Register components as tag components (accept any value for simplicity)
        // Use tag components since they accept Value::Bool(true) or Value::Map
        // For values we want to query, store them as maps
        let health_id = world.interner_mut().intern_keyword("health");
        let name_id = world.interner_mut().intern_keyword("name");
        let level_id = world.interner_mut().intern_keyword("level");
        let value_id = world.interner_mut().intern_keyword("value");

        world = world
            .register_component(ComponentSchema::new(health_id))
            .unwrap();
        world = world
            .register_component(ComponentSchema::new(name_id))
            .unwrap();
        world = world
            .register_component(ComponentSchema::new(level_id))
            .unwrap();

        // Helper to create component map with :value field
        let make_int = |val: i64| -> Value {
            let mut map = LtMap::new();
            map = map.insert(Value::Keyword(value_id), Value::Int(val));
            Value::Map(map)
        };
        let make_str = |val: &str| -> Value {
            let mut map = LtMap::new();
            map = map.insert(Value::Keyword(value_id), Value::String(val.into()));
            Value::Map(map)
        };

        // Create entities
        let (w, e1) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e1, health_id, make_int(100)).unwrap();
        world = world.set(e1, name_id, make_str("Alice")).unwrap();
        world = world.set(e1, level_id, make_int(5)).unwrap();

        let (w, e2) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e2, health_id, make_int(75)).unwrap();
        world = world.set(e2, name_id, make_str("Bob")).unwrap();
        world = world.set(e2, level_id, make_int(3)).unwrap();

        let (w, e3) = world.spawn(&LtMap::new()).unwrap();
        world = w;
        world = world.set(e3, health_id, make_int(50)).unwrap();
        world = world.set(e3, name_id, make_str("Charlie")).unwrap();
        world = world.set(e3, level_id, make_int(5)).unwrap();

        world
    }

    #[test]
    fn query_simple_pattern() {
        let mut world = setup_world();

        // Query: :where [[?e :health ?hp]] :return ?e
        // Since components are maps, we just check we get 3 entities
        let query_decl = QueryDecl {
            pattern: Pattern {
                clauses: vec![PatternClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Variable("hp".to_string()),
                    span: Span::default(),
                }],
                negations: vec![],
            },
            bindings: vec![],
            aggregates: vec![],
            group_by: vec![],
            guards: vec![],
            order_by: vec![],
            limit: None,
            return_expr: Some(Ast::Symbol("e".to_string(), Span::default())),
            span: Span::default(),
        };

        let compiled = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        let results = QueryExecutor::execute(&compiled, &world).unwrap();

        assert_eq!(results.len(), 3);
        // All results should be entity refs
        for r in &results {
            assert!(matches!(r, Value::EntityRef(_)));
        }
    }

    #[test]
    fn query_exists() {
        let mut world = setup_world();

        // Query that matches
        let query_decl = QueryDecl {
            pattern: Pattern {
                clauses: vec![PatternClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Variable("hp".to_string()),
                    span: Span::default(),
                }],
                negations: vec![],
            },
            bindings: vec![],
            aggregates: vec![],
            group_by: vec![],
            guards: vec![],
            order_by: vec![],
            limit: None,
            return_expr: None,
            span: Span::default(),
        };

        let compiled = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        assert!(QueryExecutor::exists(&compiled, &world).unwrap());
    }

    #[test]
    fn query_count() {
        let mut world = setup_world();

        let query_decl = QueryDecl {
            pattern: Pattern {
                clauses: vec![PatternClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Variable("hp".to_string()),
                    span: Span::default(),
                }],
                negations: vec![],
            },
            bindings: vec![],
            aggregates: vec![],
            group_by: vec![],
            guards: vec![],
            order_by: vec![],
            limit: None,
            return_expr: None,
            span: Span::default(),
        };

        let compiled = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        assert_eq!(QueryExecutor::count(&compiled, &world).unwrap(), 3);
    }

    #[test]
    fn query_with_multiple_clauses() {
        let mut world = setup_world();

        // Query: :where [[?e :health _] [?e :name _]] :return ?e
        // Should match entities with both components
        let query_decl = QueryDecl {
            pattern: Pattern {
                clauses: vec![
                    PatternClause {
                        entity_var: "e".to_string(),
                        component: "health".to_string(),
                        value: PatternValue::Wildcard,
                        span: Span::default(),
                    },
                    PatternClause {
                        entity_var: "e".to_string(),
                        component: "name".to_string(),
                        value: PatternValue::Wildcard,
                        span: Span::default(),
                    },
                ],
                negations: vec![],
            },
            bindings: vec![],
            aggregates: vec![],
            group_by: vec![],
            guards: vec![],
            order_by: vec![],
            limit: None,
            return_expr: Some(Ast::Symbol("e".to_string(), Span::default())),
            span: Span::default(),
        };

        let compiled = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        let results = QueryExecutor::execute(&compiled, &world).unwrap();

        // All 3 entities have both health and name
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn query_with_limit() {
        let mut world = setup_world();

        // Query: :where [[?e :health _]] :limit 2 :return ?e
        let query_decl = QueryDecl {
            pattern: Pattern {
                clauses: vec![PatternClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                }],
                negations: vec![],
            },
            bindings: vec![],
            aggregates: vec![],
            group_by: vec![],
            guards: vec![],
            order_by: vec![],
            limit: Some(2),
            return_expr: Some(Ast::Symbol("e".to_string(), Span::default())),
            span: Span::default(),
        };

        let compiled = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        let results = QueryExecutor::execute(&compiled, &world).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_one() {
        let mut world = setup_world();

        // Query: :where [[?e :health _]] :return ?e
        let query_decl = QueryDecl {
            pattern: Pattern {
                clauses: vec![PatternClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Wildcard,
                    span: Span::default(),
                }],
                negations: vec![],
            },
            bindings: vec![],
            aggregates: vec![],
            group_by: vec![],
            guards: vec![],
            order_by: vec![],
            limit: None,
            return_expr: Some(Ast::Symbol("e".to_string(), Span::default())),
            span: Span::default(),
        };

        let compiled = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        let result = QueryExecutor::execute_one(&compiled, &world).unwrap();

        assert!(result.is_some());
        assert!(matches!(result, Some(Value::EntityRef(_))));
    }

    #[test]
    fn query_return_component_value() {
        let mut world = setup_world();

        // Query: :where [[?e :health ?hp]] :return ?hp
        // hp will be a map like {:value 100}
        let query_decl = QueryDecl {
            pattern: Pattern {
                clauses: vec![PatternClause {
                    entity_var: "e".to_string(),
                    component: "health".to_string(),
                    value: PatternValue::Variable("hp".to_string()),
                    span: Span::default(),
                }],
                negations: vec![],
            },
            bindings: vec![],
            aggregates: vec![],
            group_by: vec![],
            guards: vec![],
            order_by: vec![],
            limit: None,
            return_expr: Some(Ast::Symbol("hp".to_string(), Span::default())),
            span: Span::default(),
        };

        let compiled = QueryCompiler::compile(&query_decl, world.interner_mut()).unwrap();
        let results = QueryExecutor::execute(&compiled, &world).unwrap();

        assert_eq!(results.len(), 3);
        // All results should be maps (component values)
        for r in &results {
            assert!(matches!(r, Value::Map(_)), "Expected map, got {r:?}");
        }
    }
}
