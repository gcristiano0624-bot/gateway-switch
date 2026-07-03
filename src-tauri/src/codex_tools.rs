use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum CodexToolKind {
    Function,
    Custom,
    ToolSearch,
    Namespace,
}

#[derive(Debug, Clone)]
pub struct CodexToolSpec {
    pub kind: CodexToolKind,
    pub original_name: String,
    pub original_definition: Value,
    pub chat_name: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CodexToolContext {
    pub specs: Vec<CodexToolSpec>,
    pub chat_name_to_spec: HashMap<String, usize>,
    pub seen_chat_names: HashSet<String>,
}

impl CodexToolContext {
    pub fn from_request(tools: &[Value], _input_items: &[Value]) -> Self {
        let mut ctx = CodexToolContext::default();

        for tool in tools {
            let kind = match tool.get("type").and_then(|v| v.as_str()) {
                Some("function") => CodexToolKind::Function,
                Some("custom") => CodexToolKind::Custom,
                Some("tool_search") => CodexToolKind::ToolSearch,
                Some("namespace") => CodexToolKind::Namespace,
                _ => continue,
            };

            let original_name = tool
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if original_name.is_empty() && kind != CodexToolKind::ToolSearch {
                continue;
            }

            let namespace = if kind == CodexToolKind::Namespace {
                tool.get("server_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            let chat_name = if kind == CodexToolKind::Namespace {
                let ns = namespace.as_deref().unwrap_or("");
                if original_name.contains(':') {
                    original_name.clone()
                } else {
                    format!("{}:{}", ns, original_name)
                }
            } else if kind == CodexToolKind::ToolSearch {
                "__tool_search__".to_string()
            } else {
                original_name.clone()
            };

            let chat_name = if kind == CodexToolKind::Function {
                chat_name
            } else {
                sanitize_tool_name(&chat_name)
            };

            ctx.specs.push(CodexToolSpec {
                kind,
                original_name: original_name.clone(),
                original_definition: tool.clone(),
                chat_name: chat_name.clone(),
                namespace,
            });

            ctx.chat_name_to_spec.insert(chat_name.clone(), ctx.specs.len() - 1);
            ctx.seen_chat_names.insert(chat_name);
        }

        ctx
    }

    pub fn downgrade_all_to_functions(&self) -> Vec<Value> {
        self.specs
            .iter()
            .map(|spec| self.downgrade_to_function(spec))
            .collect()
    }

    pub fn downgrade_to_function(&self, spec: &CodexToolSpec) -> Value {
        match spec.kind {
            CodexToolKind::Function => {
                let name = spec.chat_name.clone();
                let description = spec
                    .original_definition
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let parameters = spec
                    .original_definition
                    .get("parameters")
                    .cloned()
                    .unwrap_or(json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }));

                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": description,
                        "parameters": parameters,
                    }
                })
            }
            CodexToolKind::Custom => {
                let name = spec.chat_name.clone();
                let embedded = serde_json::to_string(&spec.original_definition).unwrap_or_default();
                let description = spec
                    .original_definition
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let full_desc = if description.is_empty() {
                    format!("__codex_custom_tool__:{}", embedded)
                } else {
                    format!("{}\n__codex_custom_tool__:{}", description, embedded)
                };

                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": full_desc,
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "input": {
                                    "type": "string",
                                    "description": "Tool input"
                                }
                            },
                            "required": ["input"]
                        }
                    }
                })
            }
            CodexToolKind::ToolSearch => {
                let name = spec.chat_name.clone();
                let embedded = serde_json::to_string(&spec.original_definition).unwrap_or_default();

                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": format!("__codex_tool_search__:{}", embedded),
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "Search query for tools"
                                }
                            },
                            "required": ["query"]
                        }
                    }
                })
            }
            CodexToolKind::Namespace => {
                let name = spec.chat_name.clone();
                let embedded = serde_json::to_string(&spec.original_definition).unwrap_or_default();
                let description = spec
                    .original_definition
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let full_desc = if description.is_empty() {
                    format!("__codex_namespace_tool__:{}", embedded)
                } else {
                    format!("{}\n__codex_namespace_tool__:{}", description, embedded)
                };
                let parameters = spec
                    .original_definition
                    .get("parameters")
                    .cloned()
                    .unwrap_or(json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }));

                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": full_desc,
                        "parameters": parameters,
                    }
                })
            }
        }
    }

    pub fn restore_response_item(&self, chat_name: &str, args: &Value) -> Option<Value> {
        let idx = *self.chat_name_to_spec.get(chat_name)?;
        let spec = &self.specs[idx];

        match spec.kind {
            CodexToolKind::Function => None,
            CodexToolKind::Custom => {
                let arg_str = if let Some(s) = args.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string(args).unwrap_or_default()
                };

                let input = extract_custom_tool_input(&arg_str).unwrap_or(arg_str);

                Some(json!({
                    "type": "custom_tool_call",
                    "name": spec.original_name,
                    "arguments": input,
                }))
            }
            CodexToolKind::ToolSearch => {
                let arg_str = if let Some(s) = args.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string(args).unwrap_or_default()
                };

                let query = extract_tool_search_query(&arg_str).unwrap_or(arg_str);

                Some(json!({
                    "type": "tool_search_call",
                    "query": query,
                }))
            }
            CodexToolKind::Namespace => {
                let arg_str = if let Some(s) = args.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string(args).unwrap_or_default()
                };

                Some(json!({
                    "type": "function_call",
                    "name": spec.original_name,
                    "arguments": arg_str,
                    "call_id": "",
                }))
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }
}

fn sanitize_tool_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' {
            c
        } else {
            '_'
        })
        .collect();

    if sanitized.is_empty() {
        "unknown_tool".to_string()
    } else {
        sanitized
    }
}

fn extract_custom_tool_input(arg_str: &str) -> Option<String> {
    if let Ok(val) = serde_json::from_str::<Value>(arg_str) {
        if let Some(input) = val.get("input").and_then(|v| v.as_str()) {
            return Some(input.to_string());
        }
    }
    None
}

fn extract_tool_search_query(arg_str: &str) -> Option<String> {
    if let Ok(val) = serde_json::from_str::<Value>(arg_str) {
        if let Some(query) = val.get("query").and_then(|v| v.as_str()) {
            return Some(query.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downgrade_custom_tool_to_function() {
        let tools = vec![json!({
            "type": "custom",
            "name": "apply_patch",
            "description": "Apply a patch to a file",
        })];

        let ctx = CodexToolContext::from_request(&tools, &[]);
        assert_eq!(ctx.len(), 1);

        let downgraded = ctx.downgrade_all_to_functions();
        assert_eq!(downgraded.len(), 1);

        let tool = &downgraded[0];
        assert_eq!(tool["type"], "function");
        assert_eq!(tool["function"]["name"], "apply_patch");
        assert!(tool["function"]["description"]
            .as_str()
            .unwrap()
            .contains("__codex_custom_tool__"));
        assert_eq!(tool["function"]["parameters"]["type"], "object");
        assert_eq!(
            tool["function"]["parameters"]["properties"]["input"]["type"],
            "string"
        );
    }

    #[test]
    fn test_restore_custom_tool_from_function_response() {
        let tools = vec![json!({
            "type": "custom",
            "name": "apply_patch",
            "description": "Apply a patch",
        })];

        let ctx = CodexToolContext::from_request(&tools, &[]);
        let args = json!({"input": "diff --git a/file b/file\n..."});
        let restored = ctx.restore_response_item("apply_patch", &args);

        assert!(restored.is_some());
        let item = restored.unwrap();
        assert_eq!(item["type"], "custom_tool_call");
        assert_eq!(item["name"], "apply_patch");
        assert!(item["arguments"].as_str().unwrap().contains("diff --git"));
    }

    #[test]
    fn test_namespace_preserved_round_trip() {
        let tools = vec![json!({
            "type": "namespace",
            "name": "commit",
            "server_name": "git",
            "description": "Git commit",
            "parameters": {
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                },
                "required": ["message"]
            }
        })];

        let ctx = CodexToolContext::from_request(&tools, &[]);
        assert_eq!(ctx.len(), 1);

        let downgraded = ctx.downgrade_all_to_functions();
        assert_eq!(downgraded.len(), 1);
        assert_eq!(downgraded[0]["function"]["name"], "git_commit");

        let args = json!({"message": "test commit"});
        let restored = ctx.restore_response_item("git_commit", &args);
        assert!(restored.is_some());
        let item = restored.unwrap();
        assert_eq!(item["type"], "function_call");
        assert_eq!(item["name"], "commit");
    }

    #[test]
    fn test_namespace_chat_name_is_openai_compatible() {
        let tools = vec![json!({
            "type": "namespace",
            "name": "files.read",
            "server_name": "mcp-server",
            "parameters": {"type": "object"}
        })];

        let ctx = CodexToolContext::from_request(&tools, &[]);
        let downgraded = ctx.downgrade_all_to_functions();

        assert_eq!(downgraded[0]["function"]["name"], "mcp-server_files_read");
        assert!(ctx
            .restore_response_item("mcp-server_files_read", &json!({}))
            .is_some());
    }

    #[test]
    fn test_function_tool_passthrough() {
        let tools = vec![json!({
            "type": "function",
            "name": "search",
            "description": "Search files",
            "parameters": {
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"]
            }
        })];

        let ctx = CodexToolContext::from_request(&tools, &[]);
        assert_eq!(ctx.len(), 1);

        let downgraded = ctx.downgrade_all_to_functions();
        assert_eq!(downgraded.len(), 1);
        assert_eq!(downgraded[0]["type"], "function");
        assert_eq!(downgraded[0]["function"]["name"], "search");

        let restored = ctx.restore_response_item("search", &json!({"query": "test"}));
        assert!(restored.is_none());
    }

    #[test]
    fn test_tool_search_downgrade_and_restore() {
        let tools = vec![json!({
            "type": "tool_search",
            "description": "Search for tools",
        })];

        let ctx = CodexToolContext::from_request(&tools, &[]);
        assert_eq!(ctx.len(), 1);

        let downgraded = ctx.downgrade_all_to_functions();
        assert_eq!(downgraded.len(), 1);
        assert_eq!(downgraded[0]["function"]["name"], "__tool_search__");
        assert!(downgraded[0]["function"]["description"]
            .as_str()
            .unwrap()
            .contains("__codex_tool_search__"));

        let args = json!({"query": "find files"});
        let restored = ctx.restore_response_item("__tool_search__", &args);
        assert!(restored.is_some());
        let item = restored.unwrap();
        assert_eq!(item["type"], "tool_search_call");
        assert_eq!(item["query"], "find files");
    }

    #[test]
    fn test_empty_tools() {
        let ctx = CodexToolContext::from_request(&[], &[]);
        assert!(ctx.is_empty());
        assert_eq!(ctx.downgrade_all_to_functions().len(), 0);
    }
}
