FROM alpine:3.20 as alpine

RUN apk add -U --no-cache ca-certificates tzdata

FROM alpine:3.20

EXPOSE 80 443

VOLUME /data

RUN if [[ ! -e /etc/nsswitch.conf ]] ; then echo 'hosts: files dns' > /etc/nsswitch.conf ; fi

ENV GODEBUG=netdns=go
ENV XDG_CACHE_HOME=/data
ENV CIHUB_DATABASE_DRIVER=sqlite
ENV CIHUB_DATABASE_DATASOURCE=/data/database.sqlite
ENV CIHUB_SERVER_PORT=:80
ENV CIHUB_SERVER_HOST=localhost

COPY --from=alpine /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=alpine /usr/share/zoneinfo /usr/share/zoneinfo

COPY cihub-server /usr/local/bin/cihub-server

CMD ["cihub-server", "-c", "/etc/cihub/config.yaml"]
