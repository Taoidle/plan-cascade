"""
Plan Cascade Server - FastAPI Application Entry Point

This module provides the main FastAPI application that serves as a sidecar
for the Tauri desktop application, bridging the frontend with Python core.
"""

import asyncio
from contextlib import asynccontextmanager
from typing import Optional

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from .routes import api_router
from .websocket import websocket_router
from .state import AppState


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan manager for startup and shutdown."""
    # Startup
    app.state.app_state = AppState()
    yield
    # Shutdown
    await app.state.app_state.cleanup()


def create_app() -> FastAPI:
    """Create and configure the FastAPI application."""
    app = FastAPI(
        title="Plan Cascade Server",
        description="Sidecar server for Plan Cascade Desktop Application",
        version="0.1.0",
        lifespan=lifespan,
    )

    # Configure CORS for Tauri frontend
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],  # Tauri uses custom protocol
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # Include routers
    app.include_router(api_router, prefix="/api")
    app.include_router(websocket_router)

    return app


app = create_app()


def run_server(host: str = "127.0.0.1", port: int = 8765):
    """Run the server with uvicorn."""
    import uvicorn
    uvicorn.run(app, host=host, port=port)


if __name__ == "__main__":
    run_server()
