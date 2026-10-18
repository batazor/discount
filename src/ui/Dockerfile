# discount-ui: the operator console. Context is the repository root, because the build
# generates its types from api/openapi.yaml (ADR 10).
FROM node:22-alpine AS build
WORKDIR /repo/src/ui
COPY src/ui/package.json src/ui/package-lock.json ./
RUN npm ci
COPY api /repo/api
COPY src/ui .
RUN npm run build

FROM node:22-alpine
WORKDIR /app
ENV HOST=0.0.0.0 \
    PORT=4321 \
    DISCOUNT_API_URL=http://discount:8080
COPY src/ui/package.json src/ui/package-lock.json ./
RUN npm ci --omit=dev && npm cache clean --force
COPY --from=build /repo/src/ui/dist ./dist
EXPOSE 4321
HEALTHCHECK --interval=10s --timeout=3s --retries=6 CMD wget -qO- http://localhost:4321/healthz || exit 1
CMD ["node", "./dist/server/entry.mjs"]
