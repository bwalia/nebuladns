<template>

    <div :class="'cookie-notice' + ( this.showNotice ? ' show-notice' : '')"> 

        <part flush="true">  

            <div class="cookie-layout">
   
                <div class="cookie-message">
                    This site uses <b>cookies</b> and respects your privacy. <nuxt-link to="/cookie-policy" class="u-nobr">Cookie policy</nuxt-link>     
                </div>

                <div class="cookie-button" v-on:click="rememberMyCookiePreference()">Accept</div>

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

@import "~/assets/css/variables.css";

.cookie-notice {
    position: fixed;
    bottom: 0;
    left: 0;
    width: 100vw;
    background: var(--default);
    color: #fff;
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

    a { color: #fff; }

    &.show-notice {
        transform: translateY(0);
    }

    .cookie-button {
        background: #fff;
        color: var(--default);
        line-height: 56px;
        padding: 0 1.5em;
        border-radius: 20px;
        font-weight: bold;
    }
}

</style>
