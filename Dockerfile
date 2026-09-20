FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM python:3.12-slim AS isolation-proof
RUN pip install --no-cache-dir uv
COPY --from=build /src/target/release/mcp-cli /usr/local/bin/mcpctl
RUN mkdir -p /fixtures/legacy /fixtures/modern
RUN printf '%s\n' \
  'name = "legacy-mcp"' 'version = "1.0.0"' '[runtime]' 'type = "python"' 'python = ">=3.12"' '[install]' 'strategy = "uv"' 'entrypoint = "legacy-server"' > /fixtures/legacy/mcpctl.toml \
  && printf '%s\n' \
  '[build-system]' 'requires = ["setuptools"]' 'build-backend = "setuptools.build_meta"' '[project]' 'name = "legacy-server"' 'version = "0.1.0"' 'dependencies = ["urllib3==1.26.18"]' '[project.scripts]' 'legacy-server = "server:main"' > /fixtures/legacy/pyproject.toml \
  && printf '%s\n' 'import urllib3' 'def main():' '    print(f"legacy:{urllib3.__version__}")' > /fixtures/legacy/server.py
RUN printf '%s\n' \
  'name = "modern-mcp"' 'version = "1.0.0"' '[runtime]' 'type = "python"' 'python = ">=3.12"' '[install]' 'strategy = "uv"' 'entrypoint = "modern-server"' > /fixtures/modern/mcpctl.toml \
  && printf '%s\n' \
  '[build-system]' 'requires = ["setuptools"]' 'build-backend = "setuptools.build_meta"' '[project]' 'name = "modern-server"' 'version = "0.1.0"' 'dependencies = ["urllib3==2.2.3"]' '[project.scripts]' 'modern-server = "server:main"' > /fixtures/modern/pyproject.toml \
  && printf '%s\n' 'import urllib3' 'def main():' '    print(f"modern:{urllib3.__version__}")' > /fixtures/modern/server.py
RUN MCPCTL_HOME=/state mcpctl install /fixtures/legacy \
  && MCPCTL_HOME=/state mcpctl install /fixtures/modern
CMD ["/bin/sh", "-c", "MCPCTL_HOME=/state mcpctl run legacy-mcp && MCPCTL_HOME=/state mcpctl run modern-mcp"]
