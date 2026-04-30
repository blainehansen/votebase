# podman stop --ignore postgres
# podman rm --ignore postgres

podman run --name postgres --rm -e POSTGRES_PASSWORD=dev_admin_password -e POSTGRES_USER=dev_admin_user -e POSTGRES_DB=dev_db -p 5432:5432 docker.io/library/postgres:18-alpine
