use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use etf_balancer::accounts::Portfolio;
use etf_balancer::run_balancing;

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::TemporaryRedirect()
        .header("Location", "https://github.com/gnmerritt/etf-balancer")
        .finish()
}

#[post("/balance")]
async fn balance(accounts: web::Json<Portfolio>) -> impl Responder {
    match accounts.validate() {
        None => HttpResponse::Ok().json(run_balancing(accounts.into_inner())),
        Some(err) => HttpResponse::BadRequest().json(err),
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(index).service(balance))
        .bind("127.0.0.1:8000")?
        .run()
        .await
}
