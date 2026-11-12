# Build dist first using tools/web/build.py. Original data is mounted separately.
FROM nginx:stable-alpine
ARG BUILD_REVISION=unknown
LABEL org.opencontainers.image.title="Wasm Unleashed web build"
LABEL org.opencontainers.image.source="https://github.com/vladislav-yashin/Wasm-Unleashed"
LABEL org.opencontainers.image.revision="${BUILD_REVISION}"
COPY packaging/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY dist/ /usr/share/nginx/html/
EXPOSE 8080
STOPSIGNAL SIGQUIT
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -q -T 2 -O /dev/null http://127.0.0.1:8080/healthz || exit 1
