FROM node:22-alpine

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .

ENV HOST=0.0.0.0
ENV PORT=5175
EXPOSE 5175 24679

CMD ["npm", "run", "dev"]
