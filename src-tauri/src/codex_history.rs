use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

use serde_json::{json, Value};

const MAX_RESPONSES: usize = 512;

#[derive(Default)]
pub struct CodexChatHistoryStore {
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    responses: HashMap<String, CachedResponse>,
    response_order: VecDeque<String>,
    call_index: HashMap<String, Vec<String>>,
}

#[derive(Clone, Default)]
struct CachedResponse {
    calls_by_id: HashMap<String, Value>,
    call_order: Vec<String>,
}

impl CodexChatHistoryStore {
    pub fn record_response(&self, response_id: &str, output_items: &[Value]) {
        let mut inner = match self.inner.write() {
            Ok(g) => g,
            Err(_) => return,
        };

        let mut cached = CachedResponse::default();
        for item in output_items {
            if item.get("type").and_then(|v| v.as_str()) != Some("function_call") {
                continue;
            }
            let call_id = match item.get("call_id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };
            if !cached.calls_by_id.contains_key(&call_id) {
                cached.call_order.push(call_id.clone());
            }
            cached.calls_by_id.insert(call_id.clone(), item.clone());
            inner
                .call_index
                .entry(call_id)
                .or_default()
                .push(response_id.to_string());
        }

        if cached.calls_by_id.is_empty() {
            return;
        }

        if inner.responses.contains_key(response_id) {
            inner.response_order.retain(|id| id != response_id);
        } else {
            while inner.responses.len() >= MAX_RESPONSES {
                if let Some(oldest) = inner.response_order.pop_front() {
                    if let Some(removed) = inner.responses.remove(&oldest) {
                        for cid in &removed.call_order {
                            if let Some(list) = inner.call_index.get_mut(cid) {
                                list.retain(|rid| rid != &oldest);
                                if list.is_empty() {
                                    inner.call_index.remove(cid);
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }

        inner
            .responses
            .insert(response_id.to_string(), cached);
        inner.response_order.push_back(response_id.to_string());
    }

    pub fn enrich_request(&self, body: &mut Value) -> Result<(), String> {
        let prev_resp_id = match body
            .get("previous_response_id")
            .and_then(|v| v.as_str())
        {
            Some(id) => id.to_string(),
            None => return Ok(()),
        };

        let inner = match self.inner.read() {
            Ok(g) => g,
            Err(_) => return Ok(()),
        };

        let cached = match inner.responses.get(&prev_resp_id) {
            Some(c) => c,
            None => return Ok(()),
        };

        let input = match body.get_mut("input").and_then(|v| v.as_array_mut()) {
            Some(arr) => arr,
            None => return Ok(()),
        };

        let mut tool_call_items: Vec<Value> = Vec::new();
        let mut tool_result_items: Vec<(usize, Value)> = Vec::new();

        for (idx, item) in input.iter().enumerate() {
            let itype = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if itype == "function_call_output" {
                tool_result_items.push((idx, item.clone()));
            }
        }

        if tool_result_items.is_empty() {
            return Ok(());
        }

        for (_, result_item) in &tool_result_items {
            let call_id = result_item
                .get("call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(call_item) = cached.calls_by_id.get(call_id) {
                tool_call_items.push(call_item.clone());
            }
        }

        if tool_call_items.is_empty() {
            return Ok(());
        }

        let first_result_pos = tool_result_items.first().map(|(idx, _)| *idx).unwrap_or(0);

        let mut new_input: Vec<Value> = Vec::new();
        for (idx, item) in input.drain(..).enumerate() {
            if idx == first_result_pos {
                let assistant_item = json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                });
                new_input.push(assistant_item);
                for call_item in &tool_call_items {
                    new_input.push(call_item.clone());
                }
            }
            new_input.push(item);
        }

        *body.get_mut("input").unwrap() = Value::Array(new_input);

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.inner
            .read()
            .map(|g| g.responses.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_call_item(call_id: &str, name: &str, args: &str) -> Value {
        json!({
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": args
        })
    }

    fn make_output_items(calls: &[(&str, &str, &str)]) -> Vec<Value> {
        calls
            .iter()
            .map(|(id, name, args)| make_call_item(id, name, args))
            .collect()
    }

    #[test]
    fn test_record_and_lookup() {
        let store = CodexChatHistoryStore::default();
        let items = make_output_items(&[("call_1", "test_fn", "{\"x\":1}")]);
        store.record_response("resp_1", &items);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_record_without_calls_does_not_store() {
        let store = CodexChatHistoryStore::default();
        let items = vec![json!({"type": "message", "content": "hi"})];
        store.record_response("resp_1", &items);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_lru_eviction_at_512() {
        let store = CodexChatHistoryStore::default();
        let items = make_output_items(&[("call_0", "f", "{}")]);
        for i in 0..513 {
            let resp_id = format!("resp_{}", i);
            store.record_response(&resp_id, &items);
        }
        assert_eq!(store.len(), 512);
        let first = format!("resp_0");
        let inner = store.inner.read().unwrap();
        assert!(inner.responses.get(&first).is_none());
    }

    #[test]
    fn test_enrich_request_reconstructs_tool_calls() {
        let store = CodexChatHistoryStore::default();
        let items = make_output_items(&[("call_abc", "exec_command", "{\"cmd\":\"ls\"}")]);
        store.record_response("resp_prev", &items);

        let mut body = json!({
            "model": "test",
            "previous_response_id": "resp_prev",
            "input": [
                {"type": "message", "role": "user", "content": "run it"},
                {"type": "function_call_output", "call_id": "call_abc", "output": "done"}
            ]
        });

        store.enrich_request(&mut body).unwrap();

        let input = body["input"].as_array().unwrap();
        let has_function_call = input
            .iter()
            .any(|item| item["type"] == "function_call" && item["call_id"] == "call_abc");
        assert!(has_function_call);
    }

    #[test]
    fn test_enrich_without_previous_response_id_is_noop() {
        let store = CodexChatHistoryStore::default();
        let mut body = json!({
            "model": "test",
            "input": [{"type": "message", "role": "user", "content": "hi"}]
        });
        let before = body.clone();
        store.enrich_request(&mut body).unwrap();
        assert_eq!(body, before);
    }

    #[test]
    fn test_duplicate_record_moves_to_front() {
        let store = CodexChatHistoryStore::default();
        let items = make_output_items(&[("call_1", "f", "{}")]);
        store.record_response("resp_a", &items);
        store.record_response("resp_b", &items);
        store.record_response("resp_a", &items);
        let inner = store.inner.read().unwrap();
        assert_eq!(inner.response_order.len(), 2);
        assert_eq!(
            inner.response_order.back().unwrap(),
            "resp_a"
        );
    }
}
