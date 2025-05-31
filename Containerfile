FROM python:3.11-slim
RUN pip install uv
WORKDIR /workspace
VOLUME /root/.cache/uv
ENTRYPOINT ["uv"]
