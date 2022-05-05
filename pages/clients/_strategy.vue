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
                <h2 class="u-font4 u-hbar u-but-t">{{strategy}}</h2>
            </tile-hero>

            <div class="u-pad-h u-line-tap">
                <nuxt-link to="/clients" class="u-weight-b">All Strategies</nuxt-link><b> / </b><b>{{strategy}}</b>
            </div>
            <div class="u-pad-h">
                <report-list :reportdata="items" />
            </div>

            <table>
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
                            <nuxt-link :to="document.file">
                                <span>{{document.title}}</span>
                                <svg><use xlink:href="#pdf"></use></svg>
                            </nuxt-link>
                        </td>
                    </tr>
                </tbody>
            </table>

            <table>
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
                            <nuxt-link :to="document.file">
                                <span>{{document.title}}</span>
                                <svg><use xlink:href="#pdf"></use></svg>
                            </nuxt-link>
                        </td>
                    </tr>
                </tbody>
            </table>

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
        if(!this.$auth.$storage.getUniversal('token') || !this.$route.params.strategy)
        {
            this.$router.push('/login')
        }
        let url="https://test-my.workstation.co.uk/api/webpagesEdit/"+this.$route.params.strategy;
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

        url="https://test-my.workstation.co.uk/api/getDocument";
        this.$axios.setToken(token, 'Bearer')
        this.$axios
          .get(url, {
          })
          .then((res) => {
            
            this.documents=res.data.data
            console.log(this.documents)
          })
          .catch(function (error) {
            console.log(error);
          });
    },
    data() {
        return {
            strategy: 'Volatility Arbitrage',
            url: 'volatility-arbitrage',
            target: 25,
            drawdown: 10,
            ytd: 3,
            performance: 15,
            inception: 'Dec 2021',
            items: [
                {
                    subhead: 'TAL Capital',
                    content: '<p>TAL believes a structural market inefficiency in equity capital market deals gives them a statistically significant edge that can be exploited systematically. TAL was founded in 2015 to profit from this edge. TAL has successfully run managed accounts. since 2015 monetising inefficiencies in global equity IPOs, secondary and follow-on transactions.  In July 2020 TAL launched a new Cayman fund to bring this market neutral, highly liquid strategy to more investors.</p>'
                },
                {
                    subhead: 'Portfoloio Managers',
                    content: '<a href="/">Philip Laing</a>, <a href="/">Matthew Jones</a>'
                },
                {
                    subhead: 'Strategy',
                    content: 'Systematic Equity Capital Markets'
                },
                {
                    subhead: 'Performance',
                    chart: {
                        data: {
                            labels: ["Q3'20","Q4'20","Q1'21","Q2'21","Q3'21","Q4'21","Q1'22"],
                            datasets: [
                                {
                                    label: "% increase since inception",
                                    borderColor: "#1790E3",
                                    borderWidth: 5,
                                    fill: false,
                                    data: [0, 1.6, 2.4, 2.5, 4.1, 6.2, 7, 8.8]
                                },
                                {
                                    label: "Benchmark",
                                    borderColor: "#d5d5d5",
                                    borderWidth: 5,
                                    fill: false,
                                    data: [0, 1, 2, 3, 4, 5, 6, 7]
                                }
                            ]
                        },
                        options: {
                            maintainAspectRatio: false,
                            responsive: true
                        }
                    }
                },
                {
                    subhead: 'Structure',
                    content: `
                        <p><span>Domicile:</span><br/><b>Cayman Islands</b></p>
                        <p><span>Launch date:</span><br/><b>8 July 2020</b></p>
                        <p><span>Cayman Legal Advisor:</span><br/><b>Ogier</b></p>
                        <p><span>Auditor:</span><br/><b>HLB Berman Fisher</b></p>
                        <p><span>Class A Shares ISIN:</span><br/><b>KYG6715B1014</b></p>
                        <p><span>Legal Structure:</span><br/><b>Segregated Portfolio Company (SPC) - Regulated Mutual Fund</b></p>
                        <p><span>Administrator:</span><br/><b>Charter Group Fund Administration Ltd.</b></p>
                        <p><span>Prime broke/custodian:</span><br/><b>Interactive Brokers LLC</b></p>
                        <p><span>HMRC Reporting Fund status:</span><br/><b>Accepted</b></p>
                    `
                },
                {
                    subhead: 'Terms',
                    content: `
                        <p><span>Management Fee:</span><br/><b>1.5%</b></p>
                        <p><span>Currency:</span><br/><b>GBP</b></p>
                        <p><span>Liquidity:</span><br/><b>Monthly</b></p>
                        <p><span>Performance Fee:</span><br/><b>20% of Net Asset Value appreciation. High water mark.</b></p>
                        <p><span>Minimum Investment:</span><br/><b>£100,000</b></p>
                    `
                }
            ],
            documents: [
                {
                    title: 'TAL Capital, Performance Q1 2022',
                    file: '/',
                    current: true
                },
                {
                    title: 'Terms',
                    file: '/',
                    current: true
                },
                {
                    title: 'TAL Capital, Performance Q4 2021',
                    file: '/',
                },
                {
                    title: 'TAL Capital, Performance Q3 2021',
                    file: '/',
                }
            ]
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


