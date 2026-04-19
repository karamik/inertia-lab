#!/bin/bash
# build-multiarch.sh - Build Inertia Docker images for multiple architectures

set -e

# Versions
VERSION=${VERSION:-"0.1.0"}
REGISTRY=${REGISTRY:-"ghcr.io/inertia-lab"}

# Architectures
PLATFORMS="linux/amd64,linux/arm64,linux/arm/v7"

echo "🔨 Building Inertia Docker images"
echo "Version: ${VERSION}"
echo "Platforms: ${PLATFORMS}"
echo "Registry: ${REGISTRY}"
echo ""

# Build and push multi-arch image
docker buildx build \
    --platform ${PLATFORMS} \
    --tag ${REGISTRY}/inertia:${VERSION} \
    --tag ${REGISTRY}/inertia:latest \
    --file Dockerfile \
    --push \
    .

echo ""
echo "✅ Multi-arch images built and pushed:"
echo "   - ${REGISTRY}/inertia:${VERSION}"
echo "   - ${REGISTRY}/inertia:latest"
