extern crate actix_web;
use actix_web::{server, App, Json, http, HttpRequest, HttpResponse, Responder};
extern crate etf_balancer;
use etf_balancer::accounts::{run_balancing, Accounts};

fn index(_req: HttpRequest) -> impl Responder {
    "Hello, rust"
}

fn balance(accounts: Json<Accounts>) -> impl Responder {
    HttpResponse::Ok().json(run_balancing(accounts.into_inner()))
}

fn main() {
    server::new(|| {
        App::new()
            .resource("/", |r| r.f(index))
            .resource("/balance", |r| r.method(http::Method::POST).with(balance))
    })
    .bind("127.0.0.1:8000")
    .expect("Can not bind to port 8000")
    .run();
}
