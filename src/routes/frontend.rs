use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use serde::Deserialize;

use crate::{
    app::AppState,
    auth::user::{UnauthenticatedUser, User},
    error::AppError,
    models::HoldingPosition,
    repository::Repository,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/holdings", post(add_holding))
        .route("/holdings/{id}/update", post(update_holding))
        .route("/holdings/{id}/delete", post(delete_holding))
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage;

async fn login_page() -> Result<Html<String>, AppError> {
    let html = LoginPage.render()?;
    Ok(Html(html))
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

async fn login(
    repository: Repository,
    jar: CookieJar,
    Form(request): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    let unauth_user = UnauthenticatedUser::new(request.username, request.password);
    let user = match unauth_user.authenticate(&repository).await {
        Ok(user) => user,
        Err(AppError::UserDoesNotExist) => unauth_user.register(&repository).await?,
        Err(other_err) => return Err(other_err),
    };

    let token = user.auth_token()?;
    let cookie = Cookie::build(("token", token)).http_only(true).path("/");

    Ok((jar.add(cookie), Redirect::to("/")))
}

async fn logout(jar: CookieJar) -> impl IntoResponse {
    let cookie = Cookie::build("token").path("/");
    (jar.remove(cookie), Redirect::to("/login"))
}

#[derive(Deserialize)]
struct IndexQuery {
    error: Option<String>,
}

struct HoldingView {
    id: i64,
    name: String,
    quantity: String,
    unit_value: String,
    subtotal: String,
}

struct CatalogAssetView {
    id: i64,
    label: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardPage {
    username: String,
    holdings: Vec<HoldingView>,
    total: String,
    assets: Vec<CatalogAssetView>,
    error: String,
}

fn format_money(value: f64) -> String {
    format!("R$ {value:.2}")
}

fn format_quantity(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

fn dashboard_error_message(code: Option<&str>) -> String {
    match code {
        Some("invalid_quantity") => "A quantidade deve ser maior que zero.".to_string(),
        Some("asset_not_found") => "Ativo não encontrado no catálogo.".to_string(),
        Some("holding_not_found") => "Posição não encontrada na sua carteira.".to_string(),
        _ => String::new(),
    }
}

fn redirect_app_error(err: AppError) -> Result<Redirect, AppError> {
    match err {
        AppError::InvalidQuantity => Ok(Redirect::to("/?error=invalid_quantity")),
        AppError::AssetDoesNotExist => Ok(Redirect::to("/?error=asset_not_found")),
        AppError::HoldingDoesNotExist => Ok(Redirect::to("/?error=holding_not_found")),
        other => Err(other),
    }
}

async fn index(
    repository: Repository,
    maybe_user: Option<User>,
    Query(query): Query<IndexQuery>,
) -> Result<Response, AppError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let holdings = repository.list_holdings(user.id()).await?;
    let total: f64 = holdings.iter().map(HoldingPosition::subtotal).sum();
    let assets = repository.list_assets().await?;

    let html = DashboardPage {
        username: user.username().clone(),
        holdings: holdings
            .into_iter()
            .map(|holding| HoldingView {
                id: holding.id,
                name: holding.name.clone(),
                quantity: format_quantity(holding.quantity),
                unit_value: format_money(holding.unit_value),
                subtotal: format_money(holding.subtotal()),
            })
            .collect(),
        total: format_money(total),
        assets: assets
            .into_iter()
            .map(|asset| CatalogAssetView {
                id: asset.id,
                label: format!("{} ({})", asset.name, format_money(asset.unit_value)),
            })
            .collect(),
        error: dashboard_error_message(query.error.as_deref()),
    }
    .render()?;

    Ok(Html(html).into_response())
}

#[derive(Deserialize)]
struct AddHoldingForm {
    asset_id: i64,
    quantity: f64,
}

async fn add_holding(
    repository: Repository,
    user: User,
    Form(form): Form<AddHoldingForm>,
) -> Result<Redirect, AppError> {
    match repository
        .add_holding(user.id(), form.asset_id, form.quantity)
        .await
    {
        Ok(()) => Ok(Redirect::to("/")),
        Err(err) => redirect_app_error(err),
    }
}

#[derive(Deserialize)]
struct UpdateHoldingForm {
    quantity: f64,
}

async fn update_holding(
    repository: Repository,
    user: User,
    Path(id): Path<i64>,
    Form(form): Form<UpdateHoldingForm>,
) -> Result<Redirect, AppError> {
    match repository
        .update_holding_quantity(user.id(), id, form.quantity)
        .await
    {
        Ok(Some(_)) => Ok(Redirect::to("/")),
        Ok(None) => Ok(Redirect::to("/?error=holding_not_found")),
        Err(err) => redirect_app_error(err),
    }
}

async fn delete_holding(
    repository: Repository,
    user: User,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    match repository.delete_holding(user.id(), id).await? {
        true => Ok(Redirect::to("/")),
        false => Ok(Redirect::to("/?error=holding_not_found")),
    }
}
