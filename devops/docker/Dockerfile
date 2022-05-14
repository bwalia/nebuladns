FROM node:16.14.0
WORKDIR /app
COPY ./ /app
RUN npm install
EXPOSE 3000
ENV NUXT_PORT=3000
CMD [ "npm", "run", "dev" ]