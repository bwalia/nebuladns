<template>

    <div :class="'cookie-notice' + ( this.showNotice ? ' show-notice' : '')"> 

        <part snug="true">  

            <div class="cookie-layout">
   
                <div class="cookie-message">
                    This site uses <b>cookies</b> and respects your privacy. <nuxt-link to="/privacy-policy" class="u-nobr">Privacy policy</nuxt-link>   
                </div>

                <primary-button v-on:click.native="rememberMyCookiePreference()">Accept</primary-button>

            </div>

        </part>
       
    </div>

</template>


<script>
	export default {
        ssr: false, // Disable Server Side rendering
        data () {
            return {
                showNotice: false
            }
        },
        methods: {
            rememberMyCookiePreference: function() {
                if(process.client) {
                    console.log('set cookie');

                    if(localStorage.getItem('hasAcceptedCookies')) {
                        console.log('already set');
                        this.showNotice = false;
                    } else {
                        console.log('trying to set');
                        localStorage.setItem('hasAcceptedCookies', true);
                        this.showNotice = false;
                    }
                } 
            }
        },
		mounted: async function() {

            if(process.client) {
                if(!localStorage.getItem('hasAcceptedCookies')) {
                    this.showNotice = true;
                    console.log('show notice');
                } else {
                    console.log('set?');
                }

            } else {
                console.log('not a thing');
            }
           


            
        }
	}
</script>

<style lang="scss" scoped>

.cookie-notice {
    position: fixed;
    bottom: 0;
    left: 0;
    width: 100vw;
    background: #fff;
    padding: 1.25em 0;
    z-index: 9999;
    box-shadow: 0 0 3px rgba(72, 81, 119, 0.08), 0 -3px 12px rgba(72, 81, 119, 0.08);
    transition: 0.66s ease transform;
    transform: translateY(200px);

    .cookie-layout {
        margin: 0 30px;
    
        @media (min-width: 320px ) {
            display: flex;
            align-items: center;

            .cookie-message {
                flex: 1 1 auto;
                padding-right: 2em;
            }

            #ok-button {
                flex: 0 0 auto;
            }
        }
    }

    &.show-notice {
        transform: translateY(0);
    }
}

</style>
