export default {
  
  // Target: https://go.nuxtjs.dev/config-target
  target: 'server',

  // Global page headers: https://go.nuxtjs.dev/config-head
  head: {
    title: 'odin',
    meta: [
      { charset: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { name: 'description', content: '' },
      { name: 'format-detection', content: 'telephone=no' }
    ],
    link: [
      { rel: 'icon', type: 'image/x-icon', href: '/favicon.png' },
      { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
      { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: true },
      { rel: 'stylesheet', href: 'https://fonts.googleapis.com/css2?family=Source+Sans+Pro:wght@400;900&display=swap' }
    ]
  },

  // Global CSS: https://go.nuxtjs.dev/config-css
  css: [
    "@/assets/css/variables.css",
    "@/assets/css/global.css",
    "@/assets/css/scale.scss",
    "@/assets/css/login.scss",
    "@/assets/css/utilities.css"
  ],

  // Plugins to run before rendering page: https://go.nuxtjs.dev/config-plugins
  plugins: [
  ],

  // Auto import components: https://go.nuxtjs.dev/config-components
  components: true,

  // Modules for dev and build (recommended): https://go.nuxtjs.dev/config-modules
  buildModules: [
    '@nuxtjs/dotenv',
  ],

  // Modules: https://go.nuxtjs.dev/config-modules
  modules: [
    // https://go.nuxtjs.dev/axios
    '@nuxtjs/axios',
    // https://go.nuxtjs.dev/pwa
    '@nuxtjs/pwa',
    // https://go.nuxtjs.dev/content
    '@nuxt/content',
    '@nuxtjs/auth-next',
    '@nuxtjs/recaptcha',
    '@nuxtjs/toast',
  ],
  


  // Axios module configuration: https://go.nuxtjs.dev/config-axios
  axios: {},

  // PWA module configuration: https://go.nuxtjs.dev/pwa
  pwa: {
    manifest: {
      lang: 'en'
    }
  },

  // Content module configuration: https://go.nuxtjs.dev/config-content
  content: {},

  // Build Configuration: https://go.nuxtjs.dev/config-build
  build: {
    // publicPath  : '/dist/','/'
  },
  env: {
    basePubURL: process.env.TARGET_ENV == "development" ? "http://localhost:8080"
    : process.env.TARGET_ENV == "test" ? "https://test-my.workstation.co.uk"
    : process.env.TARGET_ENV == "acceptance" ? "https://acc-my.workstation.co.uk"
    : process.env.TARGET_ENV == "integration" ? "http://int-my.workstation.co.uk"
    : "https://my.workstation.co.uk"
  },

  generate: {
    fallback: true,
    routes: [
      '/', 

      '/cookie-policy'
    ]
  },
//this.$config.apiSecretPub
  publicRuntimeConfig: {
    baseAPIURL: process.env.TARGET_ENV == "development" ? "http://localhost:8080/api"
    : process.env.TARGET_ENV == "test" ? "https://test-my.workstation.co.uk/api"
    : process.env.TARGET_ENV == "acceptance" ? "https://acc-my.workstation.co.uk/api"
    : process.env.TARGET_ENV == "integration" ? "http://int-my.workstation.co.uk/api"
    : "https://my.workstation.co.uk/api"
    ,
    apiSecretPub: process.env.API_SECRET,
    recaptcha: {
      mode: 'base',
      siteKey: process.env.CAPTCHA_SITE_KEY,
      size: 'normal',
      hideBadge: true ,
      version: 2
    }
  },

  privateRuntimeConfig: {
    apiSecretPrivate: process.env.API_SECRET
  }

}
