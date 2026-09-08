Carteira de Investimentos Fullstack com Rust

Projeto do desafio de código da DIO — uma aplicação fullstack em Rust para cadastrar ativos de investimento e acompanhar uma carteira pessoal, unindo API, banco de dados, autenticação e páginas web.

Baseado no repositório digitalinnovationone/rust-fullstack-carteira-investimentos.
O que o projeto faz

    Cadastro e login de usuários, com sessão mantida por cookie + JWT.
    Um catálogo de ativos (assets: nome + valor unitário), gerenciado via API.
    Uma carteira por usuário (holdings): cada pessoa pode adicionar ativos do catálogo à própria carteira, informando a quantidade.
    Dashboard web que lista os ativos da carteira do usuário logado, com o subtotal de cada posição e o valor total da carteira.
    Formulários para adicionar um ativo à carteira, atualizar a quantidade de uma posição e remover uma posição.

Tecnologias usadas

    Rust + Axum — servidor web e rotas
    SQLx + PostgreSQL — persistência e migrations
    Askama — templates HTML no lado do servidor
    jwt-simple (feature `pure-rust`) + axum-extra (cookies) — autenticação, sem depender de cmake/BoringSSL
    password-auth — hash de senha
    insta — testes de snapshot

Como executar

    Suba um PostgreSQL (via Docker, com `docker-compose up -d`, ou uma instância local — ajuste o `DATABASE_URL` no `.env` conforme o seu caso).
    Rode as migrations:

    cargo install sqlx-cli --no-default-features --features postgres # se ainda não tiver
    cargo sqlx migrate run

    Rode a aplicação:

    cargo run

    Cadastre alguns ativos no catálogo (só existe pela API, protegida pelo header de admin fixo do projeto base — Authorization: im-the-admin):

    curl -X POST http://localhost:3000/api/assets \
      -H "Authorization: im-the-admin" \
      -H "Content-Type: application/json" \
      -d "{\"name\": \"Bitcoin\", \"unit_value\": 350000.0}"

    Acesse http://localhost:3000. Não existe tela de cadastro separada: ao fazer login com um usuário que ainda não existe, ele é criado automaticamente.

    Se o `sqlx-cli` não estiver instalado, as migrations também podem ser aplicadas com `psql` nos arquivos em `migrations/`.

Melhoria implementada

O projeto base só tinha o CRUD de ativos exposto pela API (sem tela) e a página inicial mostrava apenas um "Hello, {usuário}" em texto puro — não existia o conceito de "quanto cada usuário possui investido".

A melhoria adicionou:

    Uma tabela holdings, ligando usuário × ativo × quantidade (migration 20260823120000_create_holdings).
    Métodos no Repository para listar, adicionar/aumentar, atualizar e remover posições da carteira (src/repository.rs).
    Um dashboard (templates/dashboard.html, servido por GET /) com a lista de ativos da carteira, subtotal por linha e o valor total calculado no servidor.
    Formulários para adicionar (POST /holdings), atualizar quantidade (POST /holdings/{id}/update) e remover (POST /holdings/{id}/delete) uma posição — cada operação valida que a quantidade é positiva (AppError::InvalidQuantity) e que a posição pertence ao usuário logado.
    Rota de logout (POST /logout), que antes não existia.

Como testar

Testes automatizados (usam um banco de testes via sqlx::test, que precisa do DATABASE_URL configurado):

cargo test
