"""
API Routes Module

Combines all API routes into a single router.
"""

from fastapi import APIRouter

from .health import router as health_router
from .execute import router as execute_router
from .status import router as status_router

api_router = APIRouter()

# Include all route modules
api_router.include_router(health_router, tags=["health"])
api_router.include_router(execute_router, tags=["execution"])
api_router.include_router(status_router, tags=["status"])
