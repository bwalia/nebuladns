<template>
    <div>

        <part>
            <nuxt-content :document="page" />
        </part>

        <!-- <cookie-notice /> -->
        
    </div>

</template>

<script>
export default {
    async asyncData({$content, params }) {
        const page = await $content('/', params.page || "index").fetch()
        return { 
            page
        }
    },
    head() {
        return {
            title: this.page.title,
            meta: [
                { hid:"og:title", property:"og:title", content: 'Odin Capital Management' },
                { hid:"description", property:"description", content: 'Access high performance strategies from skilled emerging managers' },
                { hid:"og:description", property:"og:description", content: 'Access high performance strategies from skilled emerging managers' },
                { hid:"og:image", property:"og:image", content: '' },
                { hid:"twitter:card", name:"twitter:card", content: '' }
            ]
        }
    }
}
</script>


