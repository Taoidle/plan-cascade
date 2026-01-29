"""
API Routes Module

Combines all API routes into a single router.
"""

from fastapi import APIRouter

from .health import router as health_router
from .execute import router as execute_router
from .status import router as status_router
from .analyze import router as analyze_router
from .settings import router as settings_router
from .logs import router as logs_router
from .claude_code import router as claude_code_router

api_router = APIRouter()

# Include all route modules
api_router.include_router(health_router, tags=["health"])
api_router.include_router(execute_router, tags=["execution"])
api_router.include_router(status_router, tags=["status"])
api_router.include_router(analyze_router, tags=["strategy"])
api_router.include_router(settings_router, tags=["settings"])
api_router.include_router(logs_router, tags=["logs"])
api_router.include_router(claude_code_router, tags=["claude-code"])
