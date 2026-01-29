"""
Health Check Routes

Provides endpoints for checking server health and readiness.
"""

from datetime import datetime
from typing import Dict, Any

from fastapi import APIRouter

router = APIRouter()


@router.get("/health")
async def health_check() -> Dict[str, Any]:
    """
    Health check endpoint.

    Returns server status and basic information.
    """
    return {
        "status": "healthy",
        "timestamp": datetime.utcnow().isoformat(),
        "version": "0.1.0",
        "service": "plan-cascade-server",
    }


@router.get("/ready")
async def readiness_check() -> Dict[str, Any]:
    """
    Readiness check endpoint.

    Returns whether the server is ready to accept requests.
    """
    # TODO: Add checks for dependencies (e.g., plan_cascade core availability)
    return {
        "ready": True,
        "timestamp": datetime.utcnow().isoformat(),
    }
