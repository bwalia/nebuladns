<template>
    <div>

        <svg style="display: none;">
            <defs>
                <symbol id="pdf" viewBox="0 0 16 16">
                    <title>PDF</title>
                    <polygon fill="none" stroke="#2C3033" stroke-width="1" points="3,2 3,14 13,14 13,5 10,2 "/>
                    <polyline stroke="#2c3033" fill="#2c3033" stroke-width="1" points="10,2 10,5 13,5 "/>
                    <line fill="none" stroke="#2C3033" stroke-width="1" x1="5" y1="8" x2="11" y2="8"/>
                    <line fill="none" stroke="#2C3033" stroke-width="1" x1="5" y1="9.86" x2="11" y2="9.86"/>
                    <path fill="none" stroke="#2C3033" stroke-width="1" d="M11,12"/>
                    <path fill="none" stroke="#2C3033" stroke-width="1" d="M5,12"/>
                    <rect x="2" y="12" stroke="#2c3033" fill="#2c3033" stroke-width="1" width="12" height="3"/>
                </symbol>
            </defs>
        </svg>

        <part>
            <tile-hero class="u-but-b">
                <h1 class="u-font1 u-but-b">Strategy:</h1>
                <template v-if="items && items[0] && items[0].title">
                    <h2 class="u-font4 u-hbar u-but-t">{{items[0].title}}</h2>
                </template>
            </tile-hero>
            <div class="u-pad-h u-line-tap">
                <nuxt-link to="/clients" class="u-weight-b">All Strategies</nuxt-link><b> / </b>
                
                <template v-if="items && items[0] && items[0].title">
                    <b>{{items[0].title}}</b>
                </template>
                
            </div>
            <!-- <pre>{{items}}</pre> -->
            <div class="u-pad-h">
                <report-list :reportdata="items" />
            </div>

            <!-- <table>
                <thead>
                    <tr>
                        <th>
                            Current documents
                        </th>
                    </tr>
                </thead>
                <tbody>
                    <tr v-for="(document,i) in currentDocuments" :key="i">
                        <td>
                            <a :href="document.file" target="_blank">
                                <span>{{document.metadata}}</span>
                                <svg><use xlink:href="#pdf"></use></svg>
                            </a>
                        </td>
                    </tr>
                </tbody>
            </table> -->

           <!--  <table>
                <thead>
                    <tr>
                        <th>
                            Legacy documents
                        </th>
                    </tr>
                </thead>
                <tbody>
                    <tr v-for="(document,i) in legacyDocuments" :key="i">
                        <td>
                            <a :href="document.file" target="_blank">
                                <span>{{document.metadata}}</span>
                                <svg><use xlink:href="#pdf"></use></svg>
                            </a>
                        </td>
                    </tr>
                </tbody>
            </table> -->

            <div class="u-pad-h u-line-tap">
                <nuxt-link to="/clients" class="u-weight-b">All Strategies</nuxt-link>
            </div>


        </part>

        <!-- <cookie-notice /> -->
        
    </div>

</template>

<script>

export default {
    mounted(){
        if(!this.$auth.$storage.getUniversal('loggedIn') || !this.$route.params.strategy)
        {
            this.$router.push('/login')
        }
        if(this.$auth.$storage.getUniversal('token'))
        {
            this.$auth.$storage.setUniversal('loggedIn', true)
            this.$auth.$storage.setUniversal('user', this.$auth.$storage.getUniversal('user'))
        }
        let url=process.env.baseUrl+"/api/webpagesEdit/"+this.$route.params.strategy;
        let token=this.$auth.$storage.getUniversal('token')
        this.$axios.setToken(token, 'Bearer')
        this.$axios
          .get(url, {
          })
          .then((res) => {
            
            this.items=res.data.data[0].blockList
            for(const i=1;i<res.data.data.length;i++)
            {
                this.items.push(res.data.data[i].blockList)
            }
          })
          .catch(function (error) {
            console.log(error);
          });

        // url="https://test-my.workstation.co.uk/api/getDocument";
        // this.$axios.setToken(token, 'Bearer')
        // this.$axios
        //   .get(url, {
        //   })
        //   .then((res) => {
            
        //     this.documents=res.data.data
        //     console.log(this.documents)
        //   })
        //   .catch(function (error) {
        //     console.log(error);
        //   });
    },
    data() {
        return {
            items: [],
            documents: []
        }
    },
    computed: {
        currentDocuments() {
            return this.documents.filter(el => el.current);
        },
        legacyDocuments() {
            return this.documents.filter(el => !el.current);
        },
        getServerSideProps(context) {
          const  { slug } = context.params;
          console.log(slug)
        },
        query() {
          return this.$route.query.name
        }
    },
    methods: {
        
    }
}
</script>

<style lang="css" scoped>

svg {
    width: 24px;
    height: 24px;
}

td a {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

</style>


