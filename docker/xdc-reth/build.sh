#!/bin/bash
# Build script for XDC Reth Docker image
# Supports local builds and CI/CD pipelines
# Multi-arch support: amd64 and arm64

set -e

# ============================================================================
# Configuration
# ============================================================================

DOCKER_IMAGE="${DOCKER_IMAGE:-anilchinchawale/reth-xdc}"
DOCKER_TAG="${DOCKER_TAG:-latest}"
BUILD_CONTEXT="${BUILD_CONTEXT:-../../}"  # Root of reth-xdc repo
DOCKERFILE="${DOCKERFILE:-./Dockerfile}"
PLATFORMS="${PLATFORMS:-linux/amd64,linux/arm64}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# ============================================================================
# Helper Functions
# ============================================================================

log() {
    echo -e "${GREEN}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $*"
}

warn() {
    echo -e "${YELLOW}[$(date '+%Y-%m-%d %H:%M:%S')] WARN:${NC} $*"
}

error() {
    echo -e "${RED}[$(date '+%Y-%m-%d %H:%M:%S')] ERROR:${NC} $*" >&2
    exit 1
}

usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Build XDC Reth Docker image with multi-arch support

OPTIONS:
    -t, --tag TAG           Docker tag (default: latest)
    -i, --image IMAGE       Docker image name (default: anilchinchawale/reth-xdc)
    -p, --platform PLATFORMS Build platforms (default: linux/amd64,linux/arm64)
    --single-arch           Build only for current architecture
    --push                  Push to Docker Hub after build
    --no-cache              Build without cache
    -h, --help              Show this help message

EXAMPLES:
    # Build for current architecture only (fast)
    $0 --single-arch

    # Build multi-arch and push to Docker Hub
    $0 --push

    # Build specific version tag
    $0 --tag v1.0.0 --push

    # Build with custom image name
    $0 --image myrepo/xdc-reth --tag dev

EOF
    exit 0
}

# ============================================================================
# Parse Arguments
# ============================================================================

PUSH=false
NO_CACHE=""
SINGLE_ARCH=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--tag)
            DOCKER_TAG="$2"
            shift 2
            ;;
        -i|--image)
            DOCKER_IMAGE="$2"
            shift 2
            ;;
        -p|--platform)
            PLATFORMS="$2"
            shift 2
            ;;
        --single-arch)
            SINGLE_ARCH=true
            shift
            ;;
        --push)
            PUSH=true
            shift
            ;;
        --no-cache)
            NO_CACHE="--no-cache"
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            error "Unknown option: $1 (use -h for help)"
            ;;
    esac
done

FULL_IMAGE="${DOCKER_IMAGE}:${DOCKER_TAG}"

# ============================================================================
# Validation
# ============================================================================

log "XDC Reth Docker Build Script"
log "============================="
log "Image: $FULL_IMAGE"
log "Build Context: $BUILD_CONTEXT"
log "Dockerfile: $DOCKERFILE"

# Check if Dockerfile exists
if [ ! -f "$DOCKERFILE" ]; then
    error "Dockerfile not found: $DOCKERFILE"
fi

# Check if build context exists
if [ ! -d "$BUILD_CONTEXT" ]; then
    error "Build context directory not found: $BUILD_CONTEXT"
fi

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    error "Docker is not installed or not in PATH"
fi

# Check if buildx is available for multi-arch
if [ "$SINGLE_ARCH" = false ]; then
    if ! docker buildx version &> /dev/null; then
        warn "Docker Buildx not found. Falling back to single-arch build."
        SINGLE_ARCH=true
    fi
fi

# ============================================================================
# Build Configuration
# ============================================================================

if [ "$SINGLE_ARCH" = true ]; then
    # Single architecture build (faster, for local testing)
    CURRENT_ARCH=$(uname -m)
    case "$CURRENT_ARCH" in
        x86_64)
            PLATFORMS="linux/amd64"
            ;;
        aarch64|arm64)
            PLATFORMS="linux/arm64"
            ;;
        *)
            warn "Unknown architecture: $CURRENT_ARCH. Defaulting to linux/amd64"
            PLATFORMS="linux/amd64"
            ;;
    esac
    log "Building for single architecture: $PLATFORMS"
else
    log "Building for multiple architectures: $PLATFORMS"
fi

# ============================================================================
# Docker Build
# ============================================================================

log "Starting Docker build..."
log ""

BUILD_ARGS=(
    --tag "$FULL_IMAGE"
    --file "$DOCKERFILE"
)

if [ "$SINGLE_ARCH" = false ]; then
    # Multi-arch build with buildx
    BUILD_ARGS+=(
        --platform "$PLATFORMS"
        --builder multiarch
    )
    
    # Create builder if it doesn't exist
    if ! docker buildx inspect multiarch &> /dev/null; then
        log "Creating multiarch builder..."
        docker buildx create --name multiarch --use
        docker buildx inspect --bootstrap
    else
        docker buildx use multiarch
    fi
    
    # Add push flag if requested
    if [ "$PUSH" = true ]; then
        BUILD_ARGS+=(--push)
    else
        BUILD_ARGS+=(--load)
        warn "--load only supports single platform. Using --platform ${PLATFORMS%%,*}"
        BUILD_ARGS=(
            --tag "$FULL_IMAGE"
            --file "$DOCKERFILE"
            --platform "${PLATFORMS%%,*}"
        )
    fi
fi

# Add no-cache if requested
if [ -n "$NO_CACHE" ]; then
    BUILD_ARGS+=($NO_CACHE)
fi

# Add build context
BUILD_ARGS+=("$BUILD_CONTEXT")

# Execute build
log "Build command: docker buildx build ${BUILD_ARGS[*]}"
log ""

if ! docker buildx build "${BUILD_ARGS[@]}"; then
    error "Docker build failed"
fi

log ""
log "Docker build completed successfully!"

# ============================================================================
# Push to Registry (if not already pushed by buildx)
# ============================================================================

if [ "$PUSH" = true ] && [ "$SINGLE_ARCH" = true ]; then
    log "Pushing image to registry..."
    if ! docker push "$FULL_IMAGE"; then
        error "Failed to push image to registry"
    fi
    log "Image pushed successfully: $FULL_IMAGE"
fi

# ============================================================================
# Summary
# ============================================================================

log ""
log "============================="
log "Build Summary"
log "============================="
log "Image: $FULL_IMAGE"
log "Platforms: $PLATFORMS"
log "Pushed: $PUSH"
log ""

if [ "$PUSH" = false ]; then
    log "To run the image locally:"
    log "  docker run -p 8545:8545 -p 30303:30303 -v ./data:/data $FULL_IMAGE"
    log ""
    log "To push the image:"
    log "  docker push $FULL_IMAGE"
fi

log "Build complete! 🚀"
