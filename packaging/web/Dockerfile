# Build dist first using tools/web/build.py. Original data is mounted separately.
FROM nginx:stable-alpine
COPY packaging/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY dist/ /usr/share/nginx/html/
EXPOSE 8080
