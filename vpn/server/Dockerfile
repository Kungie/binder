# Binder VPN's sign-in service: one Python file, standard library only.
FROM python:3.13-alpine
WORKDIR /app
COPY server.py .
RUN adduser -D -u 1000 vpn && mkdir /data && chown vpn /data
USER vpn
ENV BINDER_VPN_DATA=/data PYTHONUNBUFFERED=1
EXPOSE 8080
CMD ["python", "/app/server.py", "serve"]
