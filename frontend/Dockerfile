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
COPY --from=builder /app/build build/
COPY --from=builder /app/node_modules node_modules/
COPY package.json .
EXPOSE 3000
ENV NODE_ENV=production
CMD [ "node", "build" ]
