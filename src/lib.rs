use serde_json::json;
use worker::*;

mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many routes as your Worker needs! Each route will get a `Request` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .get("/", |_, _| Response::ok("Hello from Workers!"))
        .get_async("/finalize_login", |req, ctx| async move {
            console_log!("We're in finalize login");
            let url = web_sys::Url::new(&req.url().unwrap().as_str()).unwrap();
            let query_params = url.search_params();
            let code = query_params.get("code").unwrap();
            let state = query_params.get("state").unwrap();
            console_log!("The code is {code} and the state is {state}.");

            {
                let code = code.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    console_log!("We're in the spawn_local future");
                    let request_url = format!("https://github.com/login/oauth/access_token?client_id=c5b735f256dadf835133&client_secret=1f213f76072f25eb76dabb2a28c39fc93b9dd268&code={code}");
                    let request = gloo_net::http::Request::put(&request_url)
                        .query([("client_id", "c5b735f256dadf835133"),
                                // TODO: Get client secret from the environment.
                                ("client_secret", "xxxx"),
                                ("code", &code)])
                        .header("Accept", "application/json");
                    console_log!("We've constructed the request {request:?}");
                    let response = request.send().await;
                    console_log!("Response was {response:?}.");
                    let response = match response {
                        Ok(response) => response,
                        Err(err) => { console_log!("{err}"); panic!("{}", err) }
                    };
                    console_log!("We've sent the request");
                    let response_url = web_sys::Url::new(&response.url()).unwrap();
                    let response_params = response_url.search_params();
                    let response_token = response_params.get("access_token").unwrap();
                    let response_token_type = response_params.get("token_type").unwrap();
                    let response_scope = response_params.get("scope").unwrap();

                    console_log!("We have a token with {response_token}, {response_token_type}, and {response_scope}.");
                });
            }

            Response::ok(format!("We are logged in with {code} and {state}."))
        })
        .post_async("/form/:field", |mut req, ctx| async move {
            if let Some(name) = ctx.param("field") {
                let form = req.form_data().await?;
                match form.get(name) {
                    Some(FormEntry::Field(value)) => {
                        return Response::from_json(&json!({ name: value }))
                    }
                    Some(FormEntry::File(_)) => {
                        return Response::error("`field` param in form shouldn't be a File", 422);
                    }
                    None => return Response::error("Bad Request", 400),
                }
            }

            Response::error("Bad Request", 400)
        })
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .run(req, env)
        .await
}
