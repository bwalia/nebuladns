<template>
  <div>
      <part>
        <tile v-if="error.statusCode === 404">    
            <h1>Page not found</h1>    
            <p>We’re sorry, this page has moved or the address is incorrect. Try our <nuxt-link to="/">Homepage</nuxt-link></p>
        </tile>

        <tile v-else>
            <h1>An error occurred</h1>
            <p><nuxt-link to="/">Homepage</nuxt-link></p>
        </tile>
    </part>
  </div>
</template>

<script>
  export default {
    props: ['error'],
    layout: 'error'
  }
</script>