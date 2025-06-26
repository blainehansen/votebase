# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c '\l' | cat

# cargo run -p votebase_cli -- \
# 	--db-url "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db" \
# 	--ruleset-dir rulesets/three-yes-proposals \
# 	dev


# podman build -t uv-with-python .
podman run --rm -it \
	-v uv-cache:/root/.cache/uv \
	-v $(pwd):/workspace \
	uv-with-python \
	tool run -p 3.11 --with psycopg2-binary --with setuptools migra --help


# podman run --rm -it \
#   -v uv-cache:/home/blaine/.cache/uv \
#   python:3.11-slim \
#   sh -c "pip install uv && uv tool run --with psycopg2-binary --with setuptools migra --help"


# podman run --rm -it \
# 	-v uv-cache:/home/blaine/.cache/uv \
# 	-e UV_NO_MANAGED_PYTHON=true \
# 	-e UV_SYSTEM_PYTHON=true \
# 	ghcr.io/astral-sh/uv:latest \
# 	tool run -p 3.11 --with psycopg2-binary --with setuptools "migra --help"

	# sh -c "uv tool run "


# https://hub.docker.com/r/mgoltzsche/podman
# docker run --privileged mgoltzsche/podman:minimal docker run alpine:latest echo hello from nested container


# podman run --privileged quay.io/podman/stable podman run ubi8 echo hello


# podman run --privileged -v ./mycontainers:/var/lib/containers quay.io/podman/stable podman run ubi8 echo hello
