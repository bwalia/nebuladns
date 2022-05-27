<template>
    <div>
        <div class="burger" v-bind:class="{ open: isOpen }">

            <svg style="display: none;">
                <defs>
                    <symbol id="user" viewBox="0 0 24 24">
                        <title>User</title>
                        <circle fill="none" stroke="#2C3033" stroke-width="1" stroke-linecap="round" cx="12" cy="7" r="3"/>
                        <path fill="none" stroke="#2C3033" stroke-width="1" stroke-linecap="round" d="M6,17v-1c0-2.76,2.24-5,5-5h2c2.76,0,5,2.24,5,5v1" />
                    </symbol>
                </defs>
            </svg>

            <part flush="true">

                <div class="layout">

                    <logo />
                    
                    <nav>
                        <div v-for="link in links" :key="link.id">
                            <nuxt-link 
                                v-on:click.native="toggle()"
                                :to="{ path: link.url, hash: (link.anchor ? '#' + link.anchor : '')}">
                                    {{ link.label }}
                            </nuxt-link>
                        </div>                        
                        <div class="login">
                            <a href="/login" v-if="this.$store.state.auth.loggedIn">
                                Log-in
                                <svg><use xlink:href="#user"></use></svg>
                            </a>
                            <a href="/clients" v-if="this.$store.state.auth.loggedIn">
                                Strategies
                            </a>
                        </div>
                         <a  v-on:click="logout()"  v-if="this.$store.state.auth.loggedIn">
                            Log-out
                        </a>
                    </nav>

                    <div class="menu" @click="toggle()">
                        <burger-icon />
                    </div>

                </div>



            </part>
            
        </div>
        <div class="spacer"></div>
    </div>

</template>

<script>
export default {
  data () {
    return {
        isOpen: false,
        links: ''
    }
  },
  methods: {
        toggle: function() {
            this.isOpen = !this.isOpen;
        },
        closeNav: function() {
            this.isOpen = false;
            console.log('close!');
        },
        scrollToSection: function(section, offset) {
            if (this.$nuxt.$route.name === 'index') {
                window.scrollTo({
                    top: document.querySelector(section).offsetTop - offset,
                    behavior: 'smooth',
                });
            }
        },
        logout(){
            if(this.$auth.$storage.getUniversal('token')){
                this.$auth.$storage.setUniversal('token',null);
                this.$auth.$storage.setUniversal('loggedIn', false)
                this.$auth.$storage.setUniversal('user', null)
                this.$router.push('/login')
            }
        }

  },
  created() {
        this.links = [
            {label: 'Explore', url: '/explore'},
            {label: 'Investors', url: '/investors'},
            {label: 'Advisers', url: '/advisers'},
            {label: 'Contact', url: '/contact'}
        ]
    }
}
</script>

<style scoped lang="scss">

@import "~/assets/css/variables.css";
@import "~/assets/css/scale.scss";

.burger {
    position: absolute;
    width: 100%;
    z-index: 10;
    top: 0;
    left: 0;
    padding: 0.5em 0;
    background-color: white;
    transition: 0.33s background ease;

    &.open {
        background-color: var(--tint);
        @media (min-width: 768px) {
            background: none;
        }
    }
}

.layout {
    display: flex;
    flex-wrap: wrap;
    width: auto;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    margin: 0 var(--gutter);

    > .logo { 
        flex: 0 0 160px;
        @media (min-width: 768px) {
            flex: 0 0 calc( 8vw + 120px );
        }
    }
    > nav { 
        order: 3;
        flex: 0 0 100%; 
        @media (min-width: 768px) {
            order: 2;
            flex: 1 0 auto;
        }
    }
    > .menu {
        display: block;
        @media (min-width: 768px) {
            order: 3;
            display: none;
        }
    }
}

nav {
    @media (min-width: 768px) {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        flex-wrap: nowrap;
    }
}



nav a {
    display: block;
    position: relative;
    text-align: center;
    text-decoration: none;
    font-size: $scale-font2;
    font-weight: normal;
    color: var(--default);
    padding: 0.75em 1em;
    margin: 0 0.75%;
    line-height: 1.3em;
    @media (min-width: 768px) {
        font-size: $scale-default;
        flex: 0 0 auto;
    }
}

nav a:before {
    content: ' ';
    position: absolute;
    width: 0;
    left: 50%;
    bottom: 5px;
    height: 3px;
    background: var(--foil);
    margin-left: 0;
    transition: 0.2s ease all;
    border-radius: 1.5px;
}

nav a.nuxt-link-active {
    color: var(--foil);
}

nav a.nuxt-link-active:before {
    width: 30px;    
    margin-left: -15px;
    @media (min-width: 768px) {
        width: 30%;   
        margin-left: -15%; 
    }
}

nav > div:first-child a { margin-top: 2em; }

@media (min-width: 768px) {
    nav > div:first-child a { margin-top: 0; }
}

nav .login {
    flex: 0 1 auto;
    justify-content: center;
    text-align: right;
    border-radius: 21px;
    @media (min-width: 768px) {
        justify-content: flex-end;
        flex: 0.5 0 auto;
    }
}

nav .login a {
    display: flex;
    justify-content: center;
    align-items: center;
    @media (min-width: 768px) {
        justify-content: flex-end;
    }
}



.login svg {
    width: 42px;
    height: 42px;
    background: white;
    border-radius: 50%;
    margin: 0 0.5em;
    display: none;
    @media (min-width: 768px) {
        justify-content: flex-end;
        background: var(--tint);
        display: block;
    }
}

nav {
    height: 0;
    transition: height 0.66s ease;
    overflow: hidden;
    @media (min-width: 768px) {
        height: auto;
    }
}
.open nav {
    height: 100vh;
    @media (min-width: 768px) {
        height: auto;
    }
}

.spacer {
    height: 80px;
    @media (min-width: 768px) {
        height:calc(50px + 4vw);
    }
}

</style>
