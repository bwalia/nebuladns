FROM node:16.14.0
WORKDIR /app
COPY ./ /app
RUN npm install
EXPOSE 3000
ENV HOST 0.0.0.0
ENV NUXT_PORT=3000

RUN npx browserslist@latest --update-db
CMD [ "npm", "run", "dev" ]