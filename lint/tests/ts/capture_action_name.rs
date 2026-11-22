use std::path::Path;

use crate::source::with_source;
use ian_lint::rule::diagnostic::Diagnostic;
use ian_lint::rule::ts::capture_action_name::check;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    with_source(Path::new("src/page/login/context.ts"), source, check)
}

#[test]
fn an_action_name_with_the_page_and_the_method_passes() {
    let source = "\
export class UserSettingPage extends PageContext<Store> {
    async submit() {
        try {
            await Logger.capture(\"userSetting/submit\", async () => {
                await accountModule.login();
            });
        } finally {
            this.done();
        }
    }
}
class Other {
    run() {
        return Logger.capture(\"app/start\", () => {});
    }
}
Logger.capture(\"app/start\", () => {});
";
    assert!(diagnostics(source).is_empty());
}

#[test]
fn a_different_action_name_fails() {
    let source = "\
export class LoginPage extends PageContext<Store> {
    submit() {
        return Logger.capture(\"login/send\", () => {});
    }
    reset() {
        return Logger.capture(`login/${name}`, () => {});
    }
}
";
    let found = diagnostics(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].column, 31);
    assert_eq!(found[0].rule, "capture-action-name");
    assert_eq!(
        found[0].problem,
        "The action name of `Logger.capture` is not `login/submit`."
    );
    assert_eq!(
        found[0].fix,
        "Write the action name as the page name, `/`, and the name of the method."
    );
    assert_eq!(found[1].line, 6);
    assert_eq!(
        found[1].problem,
        "The action name of `Logger.capture` is not `login/reset`."
    );
}
