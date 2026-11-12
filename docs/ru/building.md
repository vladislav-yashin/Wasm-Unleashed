# Сборка и размещение

[English](../en/building.md) | **Русский**

## Что нужно

- Rust 1.95 или новее с целью `wasm32-unknown-unknown` (`rustup`);
- CLI `wasm-bindgen` версии, совпадающей с `Cargo.lock`;
- Python 3.11 или новее;
- Node 22 или новее для тестов JavaScript;
- Docker с плагином Compose — по желанию, для контейнерного образа и проверок развёртывания;
- данные игры, как описано в разделе [данные игры](game-data.md).

## Сборка и локальный запуск

```sh
make setup-web                 # добавить цель wasm и поставить подходящий wasm-bindgen CLI
make web-data GAME_DIR=/path/to/game
make build-web                 # собрать dist/ (WEB_BUILD_FLAGS=--debug — отладочная сборка)
make serve                     # отдавать dist/ и web-data/ на http://127.0.0.1:8080
```

Те же шаги без `make`:

```sh
rustup target add wasm32-unknown-unknown --toolchain stable
cargo +stable install wasm-bindgen-cli --version <версия из Cargo.lock> --locked
python3 tools/web/build.py
python3 tools/web/serve.py --data web-data
```

`tools/web/build.py` проверяет, что установленный wasm-bindgen CLI совпадает с `Cargo.lock`, собирает цель
`wasm32-unknown-unknown` и складывает браузерную оболочку в `dist/`. `make web-data` готовит данные игры
только для чтения в `web-data/`; `make serve` отдаёт обе папки и выставляет правильные MIME-типы для
модулей WebAssembly.

Запуск в контейнере:

```sh
docker compose -f packaging/web/compose.yaml up --build
```

Откройте `http://127.0.0.1:8080`. Папки по умолчанию — `dist/` и `web-data/`.

## Тесты и проверки

```sh
make test                      # тесты Rust и веба
make check-wasm                # cargo check для wasm32
make check-deploy              # проверка скриптов развёртывания и compose
make ci                        # все проверки выше
make fmt-check                 # форматирование Rust
```

Те же команды напрямую:

```sh
cargo +stable check --locked -p open-unleashed --target wasm32-unknown-unknown
cargo +stable test --locked -p nfs-formats -p nfs-net --lib
python3 -m unittest discover -s tools/web -p 'test_*.py'
node --test packaging/web/assets.test.mjs packaging/web/vfs.test.mjs
```

Тесты, которым нужны данные оригинала, пропускаются, если игра не найдена.

## Сборка для размещения

`make build-web` складывает браузерную сборку в `dist/`: `index.html`, браузерные модули и пакет
wasm-bindgen. Файлов оригинальной игры там нет. Подготовленная папка `web-data/` отдаётся отдельно и
монтируется только для чтения; промышленное развёртывание, HTTPS через Caddy и схема blue-green описаны в
[развёртывании](../../packaging/web/DEPLOY.md).

## Устройство кода

| Крейт | Назначение |
|---|---|
| `nfs-formats` | форматы файлов оригинала |
| `nfs-assets` | поиск данных игры, трассы, машины, текстуры |
| `nfs-physics` | модель машины, столкновения |
| `nfs-game` | логика гонки, ИИ, трафик, полиция, карьера, меню |
| `nfs-renderer` | рендер на wgpu, повторяющий картинку оригинала на WebGPU и WebGL2 |
| `nfs-audio`, `nfs-input`, `nfs-net` | звук, управление и обратная связь, сетевой протокол |
| `open-unleashed` | приложение игры и запуск в браузере |
| `packaging/web` | браузерная оболочка, источник ресурсов, виртуальная файловая система и сохранения |
| `tools/web` | сборка, упаковка ресурсов, локальный сервер и браузерные проверки |
