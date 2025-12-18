FROM node:22-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
# VITE_API_BASE: empty for direct backend access, "/api" for reverse proxy setup
ARG VITE_API_BASE=""
ENV VITE_API_BASE=${VITE_API_BASE}
RUN npm run build
RUN npm prune --production

FROM node:22-alpine
WORKDIR /app

# Create non-root user for security
# Using uid 1000 (standard app user ID)
RUN addgroup -g 1000 appuser && adduser -D -u 1000 -G appuser appuser

# Copy built application from builder
COPY --from=builder --chown=appuser:appuser /app/build build/
COPY --from=builder --chown=appuser:appuser /app/node_modules node_modules/
COPY --chown=appuser:appuser package.json .

EXPOSE 3000
ENV NODE_ENV=production

# Switch to non-root user
USER appuser

CMD [ "node", "build" ]
