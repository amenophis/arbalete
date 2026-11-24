//! What AppleScript asks of the pages (app/macos/Scripting.swift, decision 0017): a script run in
//! a tab's page, whose result goes back to the script that asked.

use super::*;

/// What takes a script's result: its value as text, none for `undefined`, or why it failed.
pub type ScriptDone = Box<dyn FnOnce(Result<Option<String>, String>)>;

thread_local! {
    /// The scripts sent and not answered yet, by the id of their DevTools message, with the
    /// observer of their page's answers, which goes with them.
    static RUNNING: RefCell<HashMap<i32, (ScriptDone, Option<Registration>)>> = RefCell::default();
    /// The observers of the scripts answered, which go once CEF is done calling them: dropped
    /// while it calls one, CEF reads it freed.
    static ANSWERED: RefCell<Vec<Option<Registration>>> = const { RefCell::new(Vec::new()) };
}

/// Runs `script` in the main frame of a tab's page, in the page's world, awaiting a promise it
/// gives; `done` then takes its result. False if the tab has no live page: unloaded, or never
/// shown since it opened.
pub fn execute_javascript(tab: NodeId, script: &str, done: ScriptDone) -> bool {
    let Some(host) = PAGES.with_borrow(|pages| pages.get(&tab)?.browser.host()) else {
        return false;
    };
    let id = unanswered_id();
    let registration = host.add_dev_tools_message_observer(Some(&mut ScriptObserver::new(id)));
    RUNNING.with_borrow_mut(|running| running.insert(id, (done, registration)));
    let params = json!({
        "expression": script,
        "returnByValue": true,
        "awaitPromise": true,
        "userGesture": true,
    });
    let message = json!({ "id": id, "method": "Runtime.evaluate", "params": params });
    host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    true
}

/// A script's result, from the answer to its Runtime.evaluate: a string as itself, another value
/// in JSON.
fn script_result(answer: &serde_json::Value) -> Result<Option<String>, String> {
    if let Some(error) = answer["error"]["message"].as_str() {
        return Err(error.to_string());
    }
    let result = &answer["result"];
    if let Some(exception) = result.get("exceptionDetails") {
        let text = exception["exception"]["description"]
            .as_str()
            .or(exception["text"].as_str())
            .unwrap_or("The script failed.");
        return Err(text.to_string());
    }
    Ok(match &result["result"]["value"] {
        serde_json::Value::Null if result["result"]["subtype"] != "null" => None,
        serde_json::Value::String(text) => Some(text.clone()),
        value => Some(value.to_string()),
    })
}

wrap_dev_tools_message_observer! {
    struct ScriptObserver {
        id: i32,
    }

    impl DevToolsMessageObserver {
        fn on_dev_tools_message(&self, _browser: Option<&mut Browser>, message: Option<&[u8]>) -> i32 {
            let Some(answer) = message.and_then(|m| serde_json::from_slice::<serde_json::Value>(m).ok()) else { return 0 };
            if answer["id"] != self.id {
                return 0;
            }
            if let Some((done, registration)) = RUNNING.with_borrow_mut(|running| running.remove(&self.id)) {
                ANSWERED.with_borrow_mut(|answered| answered.push(registration));
                post_task(ThreadId::UI, Some(&mut ForgetAnswered::new()));
                done(script_result(&answer));
            }
            0
        }
    }
}

wrap_task! {
    struct ForgetAnswered {}

    impl Task {
        fn execute(&self) {
            ANSWERED.with_borrow_mut(Vec::clear);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gives_a_string_as_itself_and_other_values_in_json() {
        let answer = |value| json!({ "result": { "result": value } });
        assert_eq!(
            script_result(&answer(json!({ "type": "string", "value": "Hi" }))),
            Ok(Some("Hi".into()))
        );
        assert_eq!(
            script_result(&answer(json!({ "type": "number", "value": 2 }))),
            Ok(Some("2".into()))
        );
        assert_eq!(
            script_result(&answer(
                json!({ "type": "object", "subtype": "null", "value": null })
            )),
            Ok(Some("null".into()))
        );
        assert_eq!(
            script_result(&answer(json!({ "type": "undefined" }))),
            Ok(None)
        );
    }

    #[test]
    fn tells_why_a_script_failed() {
        let answer = json!({ "result": {
            "result": { "type": "object" },
            "exceptionDetails": { "text": "Uncaught", "exception": { "description": "ReferenceError: x is not defined" } },
        } });
        assert_eq!(
            script_result(&answer),
            Err("ReferenceError: x is not defined".into())
        );
    }
}
